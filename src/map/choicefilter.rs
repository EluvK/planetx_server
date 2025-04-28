use std::collections::HashMap;

use itertools::Itertools;
use tracing::info;

use crate::operation::{
    LocateOperation, Operation, OperationResult, ResearchOperation, SurveyOperatoin,
    TargetOperation,
};

use super::{Clue, ClueConnection, MapType, SectorType, Sectors, Token, enumerator::MapEnumerator};

static MAX_CACHED_COUNT: usize = 100000;
static MAX_CACHED_COUNT_FOR_BOT: usize = 500000;

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
        // if !id.starts_with("bot-") {
        // } else {
        //     let m = MapEnumerator::new();
        //     let all = m.gen_sec(&map_type).collect();
        //     Self {
        //         map_type,
        //         id,
        //         all,
        //         ops: vec![],
        //         tokens: vec![],
        //         initialized: true,
        //     }
        // }
    }

    fn is_bot(&self) -> bool {
        self.id.starts_with("bot-")
    }

    pub fn len(&self) -> usize {
        self.all.len()
    }

    pub fn all_possibilities(&self) -> AllSectorPossibilities {
        AllSectorPossibilities::from(self.all.clone())
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
            if matches!(self.map_type, MapType::Expert) && self.ops.len() < 3 && !self.is_bot() {
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
            if cnt
                <= if self.is_bot() {
                    MAX_CACHED_COUNT_FOR_BOT
                } else {
                    MAX_CACHED_COUNT
                }
            {
                self.all = iter().collect();
                self.initialized = true;
            }
        } else {
            self.all.retain(|ss| Self::filter_op(ss, &op, &result));
            self.ops.push((op, result));
        }
        info!("{}: choices: {}", self.id, self.all.len());
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

    // try to locate the x sector
    pub fn try_locate(&self) -> Option<LocateOperation> {
        let all_possibilities = self.all_possibilities();
        let mut x_index = 0;
        let mut max_rate = 0.0;
        for s in all_possibilities.0.iter() {
            for p in s.possibilities.iter() {
                if p.sector_type == SectorType::X && p.rate > max_rate {
                    max_rate = p.rate;
                    x_index = s.index;
                }
            }
        }
        let pre_index = if x_index == 1 {
            all_possibilities.0.len()
        } else {
            x_index - 1
        };
        let next_index = if x_index == all_possibilities.0.len() {
            1
        } else {
            x_index + 1
        };
        let pre_sector_type = all_possibilities.0[pre_index - 1]
            .possibilities
            .iter()
            .filter(|x| x.sector_type != SectorType::X)
            .next();
        let next_sector_type = all_possibilities.0[next_index - 1]
            .possibilities
            .iter()
            .filter(|x| x.sector_type != SectorType::X)
            .next();
        if let (Some(pre), Some(next)) = (pre_sector_type, next_sector_type) {
            Some(LocateOperation {
                index: x_index,
                pre_sector_type: pre.sector_type.clone(),
                next_sector_type: next.sector_type.clone(),
            })
        } else {
            None
        }
    }

    pub fn effect_survey(&self, survey: &SurveyOperatoin) -> f64 {
        // if not initialized, return 0
        if !self.initialized {
            return 0.0;
        }
        // get all possible result of the survey, that is the number of surver.type between start and end
        // for example, current 1000 possibilities, 200 of them are count = 2, 300 of them are count = 3, 500 of them are count = 1.
        // the effect of the survey is 0.2 * 0.2 + 0.3 * 0.3 + 0.5 * 0.5 = 0.38
        let mut cnt = HashMap::new();
        for s in self.all.iter() {
            let count = s.get_range_type_cnt(survey.start, survey.end, &survey.sector_type);
            *cnt.entry(count).or_insert(0) += 1;
        }
        let total = self.all.len() as f64;
        let mut res = 0.0;
        for (_count, v) in cnt.iter() {
            let rate = *v as f64 / total;
            res += rate * rate;
        }
        1.0 - res
    }

    pub fn effect_target(&self, index: usize) -> f64 {
        if !self.initialized {
            return 0.0;
        }

        let all_possibilities = self.all_possibilities();

        let mut sec_rates = HashMap::<SectorType, f64>::new();
        for p in all_possibilities.0[index - 1].possibilities.iter() {
            match &p.sector_type {
                SectorType::X | SectorType::Space => {
                    *sec_rates.entry(SectorType::Space).or_insert(0.0) += p.rate
                }
                rest => *sec_rates.entry(rest.clone()).or_insert(0.0) += p.rate,
            }
        }
        let mut res = 0.0;
        for (_k, v) in sec_rates.iter() {
            res += v * v;
        }

        1.0 - res
    }

    pub fn effect_research(&self, clue: &Clue) -> f64 {
        if !self.initialized {
            return 0.0;
        }

        // try apply the clue
        let op = Operation::Research(ResearchOperation {
            index: clue.index.clone(),
        });
        let opr = OperationResult::Research(clue.clone());

        // filter the possibilities
        let cnt = self
            .all
            .iter()
            .filter(|ss| Self::filter_op(ss, &op, &opr))
            .count();
        cnt as f64 / self.all.len() as f64
    }
}

#[derive(Debug)]
pub struct SectorPossibility {
    pub sector_type: SectorType,
    pub rate: f64,
}

#[derive(Debug)]
pub struct SectorPossibilities {
    pub index: usize, // 1-based index
    pub possibilities: Vec<SectorPossibility>,
}

#[derive(Debug)]
pub struct AllSectorPossibilities(pub Vec<SectorPossibilities>);

impl From<Vec<Sectors>> for AllSectorPossibilities {
    fn from(value: Vec<Sectors>) -> Self {
        if value.is_empty() {
            return Self(vec![]);
        }
        let sector_cnt = value[0].data.len();
        let mut res = vec![];

        for i in 1..=sector_cnt {
            let mut rates = HashMap::new();
            value.iter().for_each(|s| {
                let sector = s.data[i - 1].r#type.clone();
                *rates.entry(sector.clone()).or_insert(0) += 1;
            });
            let mut possibilities = rates
                .iter()
                .map(|(k, v)| SectorPossibility {
                    sector_type: k.clone(),
                    rate: *v as f64 / value.len() as f64,
                })
                .collect::<Vec<SectorPossibility>>();
            possibilities.sort_by(|a, b| b.rate.partial_cmp(&a.rate).unwrap());

            res.push(SectorPossibilities {
                index: i,
                possibilities,
            });
        }

        Self(res)
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
        let r = cf.effect_survey(&SurveyOperatoin {
            sector_type: SectorType::Asteroid,
            start: 9,
            end: 17,
        });
        println!("effect: {}", r);
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

        println!("{:?}", cf.len());
        println!("can locate: {}", cf.can_locate());
        let all_possibilities = cf.all_possibilities();
        for s in all_possibilities.0.iter() {
            println!(
                "{:?}",
                s.possibilities
                    .iter()
                    .map(|p| format!("{} {:.3}", p.sector_type, p.rate))
                    .collect::<Vec<_>>()
            );
        }
        println!("try locate: {:?}", cf.try_locate());

        println!("res len: {}", cf.all.len());
        // for s in cf.all.iter() {
        //     println!(
        //         "{:?}",
        //         s.data.iter().map(|x| x.r#type.clone()).collect::<Vec<_>>()
        //     );
        // }
    }
}
