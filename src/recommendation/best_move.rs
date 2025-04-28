use crate::{
    map::{ChoiceFilter, Clue, ClueConnection, ClueEnum, MapType, SectorType, Token},
    operation::{
        DoPublishOperation, Operation, ReadyPublishOperation, ResearchOperation, SurveyOperatoin,
        TargetOperation,
    },
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
    pub fn dis(&self, other: &Self) -> usize {
        let dis = if self.value > other.value {
            self.value - other.value + 1
        } else {
            other.value - self.value + 1
        };
        if dis > self.max / 2 {
            self.max - dis + 2
        } else {
            dis
        }
    }
}

pub struct BestMoveInfo {
    pub stage: GameStage,
    pub map_type: MapType,
    pub start_index: SectorIndex,
    pub end_index: SectorIndex,
    pub revealed_sectors: Vec<usize>,
}

pub fn best_move(
    info: BestMoveInfo,
    clues: Vec<Clue>, // should not used the conn field
    user_state: &UserState,
    tokens: &[Token],
    choice_filter: &ChoiceFilter,
) -> Operation {
    let mut candidate_operations = vec![];

    match &info.stage {
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
    if choice_filter.can_locate()
        && (info.stage == GameStage::UserMove || info.stage == GameStage::LastMove)
    {
        if let Some(op) = choice_filter.try_locate() {
            return Operation::Locate(op);
        }
    }
    let mut moves: Vec<_> = candidate_operations
        .into_iter()
        .map(|c_op| {
            map_candidate_operations(c_op, &info, &clues, user_state, tokens, choice_filter)
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
        info!(
            "- w{:.4}|s{:2}|e{:.5}|c{}|{:?}",
            m.weight(),
            m.score,
            m.filter_effect,
            m.cost,
            m.op,
        );
    }
    info!("Best move: {:?}", moves[0].op);
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
    cost: usize,
}

impl PossibleMove {
    fn weight(&self) -> f64 {
        // [0-20]
        let effect = self.score + self.filter_effect * 10.0;
        (effect + 1.0) / self.cost as f64
    }
}

fn map_candidate_operations(
    candidate_op: CandidateOperation,
    info: &BestMoveInfo,
    clues: &[Clue],
    user_state: &UserState,
    tokens: &[Token],
    choice_filter: &ChoiceFilter,
) -> Vec<PossibleMove> {
    match candidate_op {
        CandidateOperation::Survey => {
            let start = [
                info.start_index.clone(),
                info.start_index.next(),
                info.start_index.next().next(),
            ];
            let end = [
                info.end_index.clone(),
                info.end_index.prev(),
                info.end_index.prev().prev(),
            ];
            let sector_type = [
                SectorType::DwarfPlanet,
                SectorType::Comet,
                SectorType::Asteroid,
                SectorType::Nebula,
            ];
            return start
                .iter()
                .cartesian_product(end.iter())
                .cartesian_product(sector_type.iter())
                .filter(|((start, end), sector_type)| {
                    !matches!(sector_type, SectorType::Comet)
                        || (is_prime(start.as_usize()) && is_prime(end.as_usize()))
                })
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
                        cost: 5 - (start.dis(end) + 2) / 3,
                    }
                })
                .collect::<Vec<_>>();
        }
        CandidateOperation::Target => {
            let mut candidate_index = vec![];
            let mut st = info.start_index.clone();
            while st.as_usize() != info.end_index.as_usize() {
                candidate_index.push(st);
                st = st.next();
            }
            return candidate_index
                .iter()
                .map(|index| {
                    let op = Operation::Target(TargetOperation {
                        index: index.as_usize(),
                    });
                    let filter_effect = choice_filter.effect_target(index.as_usize());
                    PossibleMove {
                        op,
                        score: 0.0, //?
                        filter_effect,
                        cost: 4,
                    }
                })
                .collect::<Vec<_>>();
        }
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
                    score: 0.0,
                    filter_effect: avg_effect,
                    cost: 2,
                });
            }
            return res;
        }
        CandidateOperation::ReadyPublish => {
            let best_shot = best_shot(info, tokens, choice_filter, 0.90);
            let number = match info.map_type {
                MapType::Standard => 1,
                MapType::Expert => 2,
            };
            let ss = best_shot
                .into_iter()
                .take(number)
                .map(|(i, s, r)| {
                    info!("ready publish best shot: {i} {s:?} {r}");
                    s
                })
                .collect();
            return vec![PossibleMove {
                op: Operation::ReadyPublish(ReadyPublishOperation { sectors: ss }),
                score: 0.0,
                filter_effect: 0.0,
                cost: 0,
            }];
        }
        CandidateOperation::DoPublish => {
            for step in 0..9 {
                let min_rate = 0.90 - step as f64 * 0.09;
                let dobest = best_shot(info, tokens, choice_filter, 0.1);
                let ss = dobest
                    .into_iter()
                    .take(1)
                    .map(|(i, s, _)| (i, s))
                    .collect::<Vec<_>>();
                if ss.is_empty() {
                    error!("No best shot available at min_rate: {min_rate}");
                    continue;
                }
                return vec![PossibleMove {
                    op: Operation::DoPublish(DoPublishOperation {
                        index: ss[0].0,
                        sector_type: ss[0].1.clone(),
                    }),
                    score: 0.0,
                    filter_effect: 0.0,
                    cost: 0,
                }];
            }
            // give a whatever result
            return vec![PossibleMove {
                op: Operation::DoPublish(DoPublishOperation {
                    index: 1,
                    sector_type: SectorType::Asteroid,
                }),
                score: 0.0,
                filter_effect: 0.0,
                cost: 0,
            }];
        }
    }
}

fn best_shot(
    info: &BestMoveInfo,
    tokens: &[Token],
    choice_filter: &ChoiceFilter,
    min_rate: f64,
) -> Vec<(usize, SectorType, f64)> {
    let usable_token = |t: &Token| !t.placed || t.secret.sector_index == 0;

    let all_possibilities = choice_filter.all_possibilities();
    let possible_sector_tokens = tokens
        .iter()
        .filter_map(|x| usable_token(x).then_some(x.r#type.clone()))
        .unique()
        .collect::<Vec<_>>();
    info!("possible sector tokens: {:?}", possible_sector_tokens);
    let guessed_sectors = tokens
        .iter()
        .filter_map(|x| {
            (x.placed && (x.secret.meeting_index != 4 || x.secret.r#type.is_some()))
                .then_some(x.secret.sector_index)
        })
        .unique()
        .collect::<Vec<_>>();
    info!("guessed sectors: {:?}", guessed_sectors);
    let mut best_shot = all_possibilities
        .0
        .into_iter()
        .filter(|sp| {
            !info.revealed_sectors.contains(&sp.index) && !guessed_sectors.contains(&sp.index)
        })
        .filter_map(|sp| {
            sp.possibilities
                .first()
                .map(|x| (sp.index, x.sector_type.clone(), x.rate))
        })
        .filter(|(_index, sector_type, _rate)| {
            possible_sector_tokens.contains(sector_type) && *_rate > min_rate
        })
        .collect::<Vec<_>>();
    best_shot.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    best_shot
}

fn is_prime(n: usize) -> bool {
    // actually, we only need to check if n is a prime number less than 18.
    // so we can just hard code the prime numbers.
    matches!(n, 2 | 3 | 5 | 7 | 11 | 13 | 17)
}
