use crate::{
    map::{ChoiceFilter, Clue, ClueConnection, ClueEnum, SectorType, Token},
    operation::{Operation, ReadyPublishOperation, ResearchOperation, SurveyOperatoin},
    room::{GameStage, UserState},
};
use itertools::Itertools;
use tracing::{error, info};

enum CandidateOperation {
    Survey,
    Target,
    Research,
    // Locate, // will locate only be sure.
    ReadyPublish,
    DoPublish,
}

// todo maybe make this a more low level struct, refactor other codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectorIndex {
    value: usize, // 1-based index
    max: usize,   // 12 or 18
}

impl SectorIndex {
    pub fn new(value: usize, max: usize) -> Self {
        if value > max {
            panic!("SectorIndex out of range");
        }
        Self { value, max }
    }
    pub fn as_usize(&self) -> usize {
        self.value
    }
    pub fn next(&self) -> Self {
        if self.value == self.max {
            return Self::new(1, self.max);
        }
        Self::new(self.value + 1, self.max)
    }
    pub fn prev(&self) -> Self {
        if self.value == 1 {
            return Self::new(self.max, self.max);
        }
        Self::new(self.value - 1, self.max)
    }
}

pub fn best_move(
    stage: GameStage,
    start_index: SectorIndex,
    end_index: SectorIndex,
    clues: Vec<Clue>, // should not used the conn field
    user_state: &UserState,
    tokens: &[Token],
    choice_filter: &ChoiceFilter,
) -> Operation {
    let mut candidate_operations = vec![];
    if choice_filter.can_locate() {
        if let Some(op) = choice_filter.try_locate() {
            return Operation::Locate(op);
        }
    }
    match stage {
        GameStage::UserMove => {
            candidate_operations.push(CandidateOperation::Survey);

            if can_research(user_state) {
                candidate_operations.push(CandidateOperation::Research);
            }
            if can_target(user_state) {
                candidate_operations.push(CandidateOperation::Target);
            }
        }
        GameStage::MeetingProposal => {
            candidate_operations.push(CandidateOperation::ReadyPublish);
        }
        GameStage::MeetingPublish => {
            candidate_operations.push(CandidateOperation::DoPublish);
        }
        stage @ (GameStage::MeetingCheck | GameStage::GameEnd) => {
            error!("{stage:?} for bot? ");
        }
        GameStage::LastMove => {
            candidate_operations.push(CandidateOperation::DoPublish);
        }
    }
    let mut moves: Vec<_> = candidate_operations
        .into_iter()
        .map(|c_op| {
            map_candidate_operations(
                c_op,
                start_index.clone(),
                end_index.clone(),
                &clues,
                user_state,
                tokens,
                choice_filter,
            )
        })
        .flatten()
        .collect();
    moves.sort_by(|a, b| b.weight().partial_cmp(&a.weight()).unwrap());
    if moves.is_empty() {
        error!("No moves available");
        // todo
        return Operation::Research(ResearchOperation { index: ClueEnum::A });
    }
    for m in &moves {
        info!("Possible move: {:?} {} {}", m.op, m.score, m.filter_effect);
    }
    return moves[0].op.clone();
}

fn can_research(user_state: &UserState) -> bool {
    if user_state
        .moves
        .last()
        .is_some_and(|x| matches!(x, Operation::Research(_)))
    {
        return false;
    }
    return true;
}

fn can_target(user_state: &UserState) -> bool {
    if user_state
        .moves
        .iter()
        .filter(|x| matches!(x, Operation::Target(_)))
        .count()
        >= 2
    {
        return false;
    }
    return true;
}

struct PossibleMove {
    op: Operation,
    score: f64,
    filter_effect: f64,
}

impl PossibleMove {
    fn weight(&self) -> f64 {
        // todo
        return self.score + self.filter_effect * 10.0;
    }
}

fn map_candidate_operations(
    candidate_op: CandidateOperation,
    start_index: SectorIndex,
    end_index: SectorIndex,
    clues: &[Clue],
    user_state: &UserState,
    tokens: &[Token],
    choice_filter: &ChoiceFilter,
) -> Vec<PossibleMove> {
    match candidate_op {
        CandidateOperation::Survey => {
            let start = [
                start_index.clone(),
                start_index.next(),
                start_index.next().next(),
            ];
            let end = [end_index.prev().prev(), end_index.prev(), end_index.clone()];
            let sector_type = [
                SectorType::Asteroid,
                SectorType::Comet,
                SectorType::DwarfPlanet,
                SectorType::Nebula,
            ];
            return start
                .iter()
                .cartesian_product(end.iter())
                .cartesian_product(sector_type.iter())
                .map(|((start, end), sector_type)| {
                    let op = SurveyOperatoin {
                        start: start.as_usize(),
                        end: end.as_usize(),
                        sector_type: sector_type.clone(),
                    };
                    let filter_effect = choice_filter.effect_survey(&op);
                    PossibleMove {
                        op: Operation::Survey(op),
                        score: 0.0,
                        filter_effect,
                    }
                })
                .collect::<Vec<_>>();
        }
        CandidateOperation::Target => {}
        CandidateOperation::Research => {
            let researched_index = user_state
                .moves
                .iter()
                .filter_map(|x| {
                    if let Operation::Research(ResearchOperation { index }) = x {
                        Some(index.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let candidate_clue_index = [
                ClueEnum::A,
                ClueEnum::B,
                ClueEnum::C,
                ClueEnum::D,
                ClueEnum::E,
                ClueEnum::F,
            ]
            .into_iter()
            .filter(|x| !researched_index.contains(x))
            .collect::<Vec<_>>();

            let mut res = vec![];
            for index in &candidate_clue_index {
                if researched_index.contains(index) {
                    continue;
                }
                let Some(clue) = clues.iter().find(|x| x.index == *index) else {
                    error!("Clue not found: {index:?}");
                    continue;
                };
                let avg_effect = [
                    ClueConnection::AllAdjacent,
                    ClueConnection::NotAdjacent,
                    ClueConnection::OneOpposite,
                    ClueConnection::NotOpposite,
                    ClueConnection::AllInRange(4),
                    ClueConnection::NotInRange(3),
                ]
                .iter()
                .map(|conn| {
                    let clue = Clue {
                        index: clue.index.clone(),
                        subject: clue.subject.clone(),
                        object: clue.object.clone(),
                        conn: conn.clone(),
                    };
                    choice_filter.effect_research(&clue)
                })
                .fold(0.0, |acc, x| acc + x)
                    / 6.0;
                res.push(PossibleMove {
                    op: Operation::Research(ResearchOperation {
                        index: clue.index.clone(),
                    }),
                    score: avg_effect,
                    filter_effect: 0.0,
                });
            }
            return res;
        }
        CandidateOperation::ReadyPublish => {
            return vec![PossibleMove {
                op: Operation::ReadyPublish(ReadyPublishOperation { sectors: vec![] }),
                score: 0.0,
                filter_effect: 0.0,
            }];
        }
        CandidateOperation::DoPublish => {}
    }
    vec![]
}
