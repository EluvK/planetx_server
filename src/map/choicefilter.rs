use itertools::Itertools;
use tracing::info;

use crate::operation::{
    LocateOperation, Operation, OperationResult, SurveyOperatoin, TargetOperation,
};

use super::{ClueConnection, MapType, SectorType, Sectors, Token, enumerator::MapEnumerator};

static MAX_CACHED_COUNT: usize = 100000;

#[derive(Debug, Clone)]
pub struct ChoiceFilter {
    map_type: MapType,
    id: String,
    pub all: Vec<Sectors>,
    ops: Vec<(Operation, OperationResult)>,
    tokens: Vec<Token>,
    pub initialized: bool,
}

impl ChoiceFilter {
    pub fn new(map_type: MapType, id: String) -> Self {
        Self {
            map_type,
            id,
            all: vec![],
            ops: vec![],
            tokens: vec![],
            initialized: false,
        }
    }

    pub fn possibilities(&self) -> usize {
        self.all.len()
    }

    pub fn update_tokens(&mut self, token: &[Token]) {
        // not initialized
        if !self.initialized {
            // cached tokens if not enough operations to start filtering
            self.tokens = token.to_vec();
            return;
        }
        self.all
            .retain(|ss| token.iter().all(|t| Self::filter_token(ss, t)));
    }

    pub fn add_operation(&mut self, op: Operation, result: OperationResult) {
        // not initialized
        if !self.initialized {
            self.ops.push((op, result));
            if matches!(self.map_type, MapType::Expert) && self.ops.len() < 3 {
                // expert map, no need to filter
                return;
            }
            // if self.ops.len() < 2 {
            //     return;
            // }
            // at least 2 operations
            let m = MapEnumerator::new();
            let iter = || {
                m.gen_sec(&self.map_type).filter(|ss| {
                    self.ops
                        .iter()
                        .all(|(op, opr)| Self::filter_op(ss, op, opr))
                        && self.tokens.iter().all(|t| Self::filter_token(ss, t))
                })
            };
            let cnt = iter().count();
            if cnt <= MAX_CACHED_COUNT {
                self.all = iter().collect();
                self.initialized = true;
            }
        } else {
            self.all.retain(|ss| Self::filter_op(ss, &op, &result));
            self.ops.push((op, result));
        }
        info!("{}: possibilities: {}", self.id, self.all.len());
    }

    fn filter_token(ss: &Sectors, token: &Token) -> bool {
        if !token.placed {
            return true;
        }
        if token.secret.r#type.is_none() {
            return true;
        }
        if token.secret.meeting_index == 4 {
            ss.data[token.secret.sector_index - 1].r#type != token.r#type
        } else {
            ss.data[token.secret.sector_index - 1].r#type == token.r#type
        }
    }

    fn filter_op(ss: &Sectors, op: &Operation, opr: &OperationResult) -> bool {
        match (op, opr) {
            (
                Operation::Survey(SurveyOperatoin {
                    sector_type,
                    start,
                    end,
                }),
                OperationResult::Survey(cnt),
            ) => ss.get_range_type_cnt(*start, *end, sector_type) == *cnt,
            (Operation::Target(TargetOperation { index }), OperationResult::Target(r#type)) => {
                match r#type {
                    SectorType::Space => {
                        ss.data[*index - 1].r#type == SectorType::Space
                            || ss.data[*index - 1].r#type == SectorType::X
                    }
                    _ => ss.data[*index - 1].r#type == *r#type,
                }
            }
            (Operation::Research(_), OperationResult::Research(clue)) => match clue.conn {
                ClueConnection::AllAdjacent => {
                    for sindex in ss
                        .data
                        .iter()
                        .filter_map(|x| (x.r#type == clue.subject).then_some(x.index))
                    {
                        if ss.prev(sindex).r#type != clue.object
                            && ss.next(sindex).r#type != clue.object
                        {
                            return false;
                        }
                    }
                    true
                }
                ClueConnection::OneAdjacent => ss
                    .data
                    .iter()
                    .filter_map(|x| (x.r#type == clue.subject).then_some(x.index))
                    .any(|sindex| {
                        ss.prev(sindex).r#type == clue.object
                            || ss.next(sindex).r#type == clue.object
                    }),
                ClueConnection::NotAdjacent => ss
                    .data
                    .iter()
                    .filter_map(|x| (x.r#type == clue.subject).then_some(x.index))
                    .all(|sindex| {
                        ss.prev(sindex).r#type != clue.object
                            && ss.next(sindex).r#type != clue.object
                    }),
                ClueConnection::OneOpposite => ss.data.iter().any(|x| {
                    x.r#type == clue.subject && ss.opposite(x.index).r#type == clue.object
                }),
                ClueConnection::NotOpposite => ss.data.iter().all(|x| {
                    x.r#type != clue.subject || ss.opposite(x.index).r#type != clue.object
                }),
                ClueConnection::AllInRange(range) => ss
                    .data
                    .iter()
                    .filter(|&x| x.r#type == clue.subject)
                    .all(|x| ss.check_range_exist(x.index, &clue.object, range)),
                ClueConnection::NotInRange(range) => ss
                    .data
                    .iter()
                    .filter(|&x| x.r#type == clue.subject)
                    .all(|x| !ss.check_range_exist(x.index, &clue.object, range)),
            },
            (
                Operation::Locate(LocateOperation {
                    index,
                    pre_sector_type,
                    next_sector_type,
                }),
                OperationResult::Locate(r),
            ) => {
                if *r {
                    ss.data[*index - 1].r#type == SectorType::X
                        && ss.prev(*index).r#type == *pre_sector_type
                        && ss.next(*index).r#type == *next_sector_type
                } else {
                    true
                }
            }
            (Operation::ReadyPublish(_), OperationResult::ReadyPublish(_)) => true,
            (Operation::DoPublish(_), OperationResult::DoPublish(_)) => true,
            _ => true,
        }
    }

    pub fn can_locate(&self) -> bool {
        // can locate if all the possibilities of x are in the same sector and the adjacent sectors is only one type
        self.initialized
            && self
                .all
                .iter()
                .map(|s| {
                    // first find only x sectors
                    let x_index = s
                        .data
                        .iter()
                        .filter(|x| x.r#type == SectorType::X)
                        .map(|x| x.index)
                        .next()
                        .unwrap();
                    // then find the adjacent sectors
                    let mut adjacent = vec![];
                    for i in 1..=s.data.len() {
                        if s.prev(i).r#type == SectorType::X || s.next(i).r#type == SectorType::X {
                            adjacent.push(s.data[i - 1].r#type.clone());
                        }
                    }
                    (adjacent, x_index)
                })
                .all_equal()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        map::{Clue, ClueEnum, SecretToken, SectorType},
        operation::ResearchOperation,
    };

    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_filter2() {
        let mut cf = ChoiceFilter::new(MapType::Expert, "test".to_string());
        macro_rules! survey {
            ($t:expr, $start:expr, $end:expr, $cnt:expr) => {
                cf.add_operation(
                    Operation::Survey(SurveyOperatoin {
                        sector_type: $t,
                        start: $start,
                        end: $end,
                    }),
                    OperationResult::Survey($cnt),
                );
            };
        }

        survey!(SectorType::DwarfPlanet, 1, 9, 3);
        survey!(SectorType::Comet, 3, 11, 1);
        survey!(SectorType::DwarfPlanet, 5, 13, 4);
        survey!(SectorType::Comet, 7, 13, 1);
        survey!(SectorType::Asteroid, 9, 17, 1);
        survey!(SectorType::Asteroid, 12, 2, 2);
        survey!(SectorType::Nebula, 16, 4, 2);
        survey!(SectorType::Comet, 17, 7, 1);
        survey!(SectorType::Nebula, 1, 9, 1);
        survey!(SectorType::Comet, 3, 11, 1);
        survey!(SectorType::DwarfPlanet, 5, 11, 3);

        cf.add_operation(
            Operation::Target(TargetOperation { index: 11 }),
            OperationResult::Target(SectorType::Comet),
        );

        cf.add_operation(
            Operation::Target(TargetOperation { index: 3 }),
            OperationResult::Target(SectorType::Space),
        );

        macro_rules! research {
            ($i:expr,  $conn:expr, $s:expr, $o:expr) => {
                cf.add_operation(
                    Operation::Research(ResearchOperation { index: $i }),
                    OperationResult::Research(Clue {
                        index: $i,
                        conn: $conn,
                        subject: $s,
                        object: $o,
                    }),
                );
            };
        }

        research!(
            ClueEnum::X1,
            ClueConnection::AllAdjacent,
            SectorType::X,
            SectorType::Comet
        );

        cf.update_tokens(&[
            Token {
                placed: true,
                secret: SecretToken {
                    user_id: "xxx".to_owned(),
                    user_index: 1,
                    sector_index: 17,
                    meeting_index: 3,
                    r#type: Some(SectorType::Asteroid),
                },
                r#type: SectorType::Asteroid,
            },
            Token {
                placed: true,
                secret: SecretToken {
                    user_id: "xxx".to_owned(),
                    user_index: 1,
                    sector_index: 11,
                    meeting_index: 4,
                    r#type: Some(SectorType::DwarfPlanet),
                },
                r#type: SectorType::DwarfPlanet,
            },
        ]);

        println!("{:?}", cf.possibilities());
        println!("can locate: {}", cf.can_locate());
        for s in cf.all.iter() {
            println!(
                "{:?}",
                s.data.iter().map(|x| x.r#type.clone()).collect::<Vec<_>>()
            );
        }
    }
}
