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
                ss.data[*index - 1].r#type == *r#type
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
            (Operation::Locate(LocateOperation { index, .. }), OperationResult::Locate(r)) => {
                if *r {
                    ss.data[*index - 1].r#type == SectorType::X
                } else {
                    ss.data[*index - 1].r#type != SectorType::X
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
                            adjacent.push(s.data[i].r#type.clone());
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
        map::{Clue, SecretToken, SectorType},
        operation::ResearchOperation,
    };

    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_filter() {
        let mut cf = ChoiceFilter::new(MapType::Expert, "test".to_string());
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::DwarfPlanet,
                start: 1,
                end: 9,
            }),
            OperationResult::Survey(4),
        );

        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::Comet,
                start: 3,
                end: 11,
            }),
            OperationResult::Survey(1),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Research(ResearchOperation {
                index: crate::map::ClueEnum::A,
            }),
            OperationResult::Research(Clue {
                index: crate::map::ClueEnum::A,
                conn: ClueConnection::OneOpposite,
                subject: SectorType::Comet,
                object: SectorType::DwarfPlanet,
            }),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Research(ResearchOperation {
                index: crate::map::ClueEnum::X1,
            }),
            OperationResult::Research(Clue {
                index: crate::map::ClueEnum::X1,
                conn: ClueConnection::NotAdjacent,
                subject: SectorType::X,
                object: SectorType::Space,
            }),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::DwarfPlanet,
                start: 6,
                end: 14,
            }),
            OperationResult::Survey(1),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::Asteroid,
                start: 8,
                end: 16,
            }),
            OperationResult::Survey(2),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Research(ResearchOperation {
                index: crate::map::ClueEnum::B,
            }),
            OperationResult::Research(Clue {
                index: crate::map::ClueEnum::B,
                conn: ClueConnection::AllInRange(5),
                subject: SectorType::Nebula,
                object: SectorType::Comet,
            }),
        );
        println!("{:?}", cf.possibilities());

        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::DwarfPlanet,
                start: 11,
                end: 1,
            }),
            OperationResult::Survey(0),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::Asteroid,
                start: 13,
                end: 3,
            }),
            OperationResult::Survey(4),
        );
        println!("{:?}", cf.possibilities());

        cf.add_operation(
            Operation::Research(ResearchOperation {
                index: crate::map::ClueEnum::C,
            }),
            OperationResult::Research(Clue {
                index: crate::map::ClueEnum::C,
                conn: ClueConnection::NotAdjacent,
                subject: SectorType::DwarfPlanet,
                object: SectorType::Nebula,
            }),
        );
        println!("{:?}", cf.possibilities());

        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::Nebula,
                start: 16,
                end: 6,
            }),
            OperationResult::Survey(1),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Research(ResearchOperation {
                index: crate::map::ClueEnum::X2,
            }),
            OperationResult::Research(Clue {
                index: crate::map::ClueEnum::X2,
                conn: ClueConnection::AllAdjacent,
                subject: SectorType::X,
                object: SectorType::Comet,
            }),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Research(ResearchOperation {
                index: crate::map::ClueEnum::D,
            }),
            OperationResult::Research(Clue {
                index: crate::map::ClueEnum::D,
                conn: ClueConnection::OneOpposite,
                subject: SectorType::Comet,
                object: SectorType::Space,
            }),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::Comet,
                start: 2,
                end: 7,
            }),
            OperationResult::Survey(0),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::Nebula,
                start: 4,
                end: 12,
            }),
            OperationResult::Survey(1),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Research(ResearchOperation {
                index: crate::map::ClueEnum::E,
            }),
            OperationResult::Research(Clue {
                index: crate::map::ClueEnum::E,
                conn: ClueConnection::OneAdjacent,
                subject: SectorType::DwarfPlanet,
                object: SectorType::Space,
            }),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Survey(SurveyOperatoin {
                sector_type: SectorType::Nebula,
                start: 7,
                end: 15,
            }),
            OperationResult::Survey(1),
        );
        println!("{:?}", cf.possibilities());
        cf.add_operation(
            Operation::Research(ResearchOperation {
                index: crate::map::ClueEnum::F,
            }),
            OperationResult::Research(Clue {
                index: crate::map::ClueEnum::F,
                conn: ClueConnection::NotAdjacent,
                subject: SectorType::Nebula,
                object: SectorType::Asteroid,
            }),
        );
        println!("{:?}", cf.possibilities());

        // for s in cf.all.iter() {
        //     println!("{:?}", s);
        // }

        // cf.add_operation(
        //     Operation::Target(TargetOperation { index: 10 }),
        //     OperationResult::Target(SectorType::Nebula),
        // );
        // println!("{:?}", cf.possibilities());

        // 11 comet 12 x 13 asteroid
        cf.update_tokens(&[
            Token {
                placed: true,
                secret: SecretToken {
                    user_id: "xxx".to_owned(),
                    user_index: 1,
                    sector_index: 10,
                    meeting_index: 3,
                    r#type: Some(SectorType::Nebula),
                },
                r#type: SectorType::Nebula,
            },
            // Token {
            //     placed: true,
            //     secret: SecretToken {
            //         user_id: "xxx".to_owned(),
            //         user_index: 1,
            //         sector_index: 2,
            //         meeting_index: 4,
            //         r#type: Some(SectorType::DwarfPlanet),
            //     },
            //     r#type: SectorType::DwarfPlanet,
            // },
        ]);
        println!("can locate: {}", cf.can_locate());
        for s in cf.all.iter() {
            println!(
                "{:?}",
                s.data.iter().map(|x| x.r#type.clone()).collect::<Vec<_>>()
            );
        }
    }
}
