use core::panic;

use rand::{Rng, SeedableRng, rngs::SmallRng};
use serde::{Deserialize, Serialize};

use super::{
    MapType,
    model::{SectorType, Sectors},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Clue {
    pub index: ClueEnum,
    pub subject: SectorType,
    pub object: SectorType,
    pub conn: ClueConnection,
}

impl std::fmt::Display for Clue {
    #[rustfmt::skip]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.subject == SectorType::X {
            return match self.conn {
                ClueConnection::AllAdjacent => write!(f, "{} 和 {} 相邻", self.subject, self.object),
                ClueConnection::OneAdjacent => write!(f, "{} 和 {} 相邻", self.subject, self.object),
                ClueConnection::NotAdjacent => write!(f, "{} 不和 {} 相邻", self.subject, self.object),
                ClueConnection::OneOpposite => write!(f, "{} 和 {} 正对", self.subject, self.object),
                ClueConnection::NotOpposite => write!(f, "{} 不和 {} 正对", self.subject, self.object),
                ClueConnection::AllInRange(n) => write!(f, "{} 在 {} 的 {} 格范围内", self.subject, self.object, n),
                ClueConnection::NotInRange(n) => write!(f, "{} 不在 {} 的 {} 格内", self.subject, self.object, n),
            };
        }
        match self.conn {
            ClueConnection::AllAdjacent => write!(f, "所有 {} 和 {} 相邻", self.subject, self.object),
            ClueConnection::OneAdjacent => write!(f, "至少一个 {} 和 {} 相邻", self.subject, self.object),
            ClueConnection::NotAdjacent => write!(f, "没有 {} 和 {} 相邻", self.subject, self.object),
            ClueConnection::OneOpposite => write!(f, "至少一个 {} 和 {} 正对", self.subject, self.object),
            ClueConnection::NotOpposite => write!(f, "没有 {} 和 {} 正对", self.subject, self.object),
            ClueConnection::AllInRange(n) => match self.object == self.subject {
                true => write!(f, "所有 {} 都在一个长度为 {} 的区间内", self.subject, n),
                false => write!(f, "所有 {} 在 {} 的 {} 格范围内", self.subject, self.object, n),
            },
            ClueConnection::NotInRange(n) => write!(f, "没有 {} 在 {} 的 {} 格内", self.subject, self.object, n),
        }
    }
}

impl Clue {
    pub fn as_secret(&self) -> String {
        if self.object == self.subject || self.object == SectorType::Space {
            return format!("{}", self.subject);
        }
        format!("{} {}", self.subject, self.object)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ClueEnum {
    A,
    B,
    C,
    D,
    E,
    F,
    X1,
    X2,
}
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ClueSecret {
    pub index: ClueEnum,
    pub secret: String,
}
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ClueDetail {
    pub index: ClueEnum,
    pub detail: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClueConnection {
    AllAdjacent, // all
    OneAdjacent, // at least one
    NotAdjacent,
    OneOpposite, // at least one
    NotOpposite,
    AllInRange(usize), // object can be None
    NotInRange(usize),
}

// examples:
// 所有 a 和 b 相邻
// 至少一个 a 和 b 相邻
// 没有 a 与 b 相邻
// 至少一个 a 和 b 正对
// 没有 a 和 b 正对
// 所有 a 在 b 的三格范围内 2/3/4
// 所有 a 都在 ?格内 彗6/小7/气体云3|6
// 没有 a 在 b 的两格内 2/3

// 每个矮都和一个空相邻 ，（ 没有 object ）X7D7-D ， use {dwarf, None, AllAdjacent}
// 没有彗星在彗星的6格内，（ 没有 object ）R6B8-C， use {comet, None, NotInRange(6)}
// ?在一组连续的扇形区域中共有3个矮行星，（ 没有 object ）U7K7-D

// map Z5D6 is very special.

pub struct ClueGenerator {
    seed: u64,
    rng: SmallRng,
    sectors: Sectors,
    map_type: MapType,
}

impl ClueGenerator {
    pub fn new(seed: u64, sectors: Sectors, map_type: MapType) -> Self {
        Self {
            seed,
            rng: SmallRng::seed_from_u64(seed),
            sectors,
            map_type,
        }
    }

    pub fn generate_clues(&mut self) -> anyhow::Result<(Vec<Clue>, Vec<Clue>)> {
        let mut res = vec![];

        while res.len() < 6 {
            let index = match res.len() {
                0 => ClueEnum::A,
                1 => ClueEnum::B,
                2 => ClueEnum::C,
                3 => ClueEnum::D,
                4 => ClueEnum::E,
                5 => ClueEnum::F,
                _ => panic!("clue index out of range"),
            };
            let subject = self.get_rand_type(false, false);
            let object = self.get_rand_type(true, false);

            let conn = self.get_rand_conn(false);
            if !self.check_clue(&res, &subject, &object, &conn) {
                continue;
            }
            res.push(Clue {
                index,
                subject,
                object,
                conn,
            });
        }
        let mut xres = vec![];
        let mut cnt = 0;
        while xres.is_empty() || !check_x_space_only(&res, &xres, &self.sectors).is_empty() {
            xres.clear();
            while xres.len() < self.map_type.xclue_points().len() {
                let index = match xres.len() {
                    0 => ClueEnum::X1,
                    1 => ClueEnum::X2,
                    _ => panic!("clue index out of range"),
                };
                let subject = SectorType::X;
                let object = self.get_rand_type(true, false);

                let conn = self.get_rand_conn(true);
                if !self.check_clue(&xres, &subject, &object, &conn) {
                    continue;
                }
                xres.push(Clue {
                    index,
                    subject,
                    object,
                    conn,
                });
            }
            cnt += 1;
            if cnt > 100 {
                return Err(anyhow::anyhow!("x clue too much try"));
            }
        }

        Ok((res, xres))
    }

    fn get_rand_type(&mut self, allow_space: bool, allow_x: bool) -> SectorType {
        loop {
            let rand: SectorType = self.rng.random();
            if !allow_space && rand == SectorType::Space {
                continue;
            }
            if !allow_x && rand == SectorType::X {
                continue;
            }
            return rand;
        }
    }

    fn get_rand_conn(&mut self, is_x: bool) -> ClueConnection {
        let easy = matches!(self.map_type, MapType::Standard) || is_x;

        let distributions = [
            (200, ClueConnection::AllAdjacent),
            (10, ClueConnection::OneAdjacent),
            (16, ClueConnection::NotAdjacent),
            (10, ClueConnection::OneOpposite),
            (12, ClueConnection::NotOpposite),
            (
                7,
                ClueConnection::AllInRange(self.rng.random_range(if easy { 2..=4 } else { 4..=6 })),
            ),
            (
                64,
                ClueConnection::NotInRange(self.rng.random_range(if easy { 3..=4 } else { 2..=3 })),
            ),
        ];

        // 计算总和
        let sum: i32 = distributions.iter().map(|(weight, _)| *weight).sum();

        // 生成随机数
        let mut r = self.rng.random_range(0..sum);

        // 根据权重选择
        for (weight, conn) in distributions.iter() {
            if r < *weight {
                return conn.clone();
            }
            r -= *weight;
        }
        assert!(false, "should not reach here"); // 理论上不会执行到这里
        // 默认情况（理论上不会执行到这里）
        distributions.last().unwrap().1.clone()
    }

    fn check_clue(
        &self,
        clues: &[Clue],
        subject: &SectorType,
        object: &SectorType,
        conn: &ClueConnection,
    ) -> bool {
        // same clue secret
        let try_secret = if object == subject || *object == SectorType::Space {
            format!("{}", subject)
        } else {
            format!("{} {}", subject, object)
        };
        for clue in clues {
            if clue.as_secret() == try_secret {
                return false;
            }

            // same clue pair
            if clue.subject == *object && clue.object == *subject {
                return false;
            }
            if clue.subject == *object
                && clue.object == *subject
                && std::mem::discriminant(conn) == std::mem::discriminant(&clue.conn)
            {
                return false;
            }
        }

        // too much clue for same type
        if clues
            .iter()
            .filter(|x| x.subject == *subject || x.object == *subject)
            .count()
            >= 3
        {
            return false;
        }
        if clues.iter().filter(|x| x.subject == *subject).count() >= 2 {
            return false;
        }
        if clues.iter().filter(|x| x.object == *object).count() >= 2 {
            return false;
        }
        if clues
            .iter()
            .filter(|x| x.subject == *object || x.object == *object)
            .count()
            >= 3
        {
            return false;
        }

        if *subject == SectorType::Space {
            panic!("no possible check algorithm");
        }

        match conn {
            ClueConnection::AllAdjacent => match (subject, object) {
                (SectorType::Comet, SectorType::Comet) => false, // op clue show commets are 2 && 3
                (SectorType::Asteroid, SectorType::Asteroid)
                | (SectorType::Nebula, SectorType::Space) => false, // not very useful
                // (SectorType::DwarfPlanet, SectorType::DwarfPlanet) => return false, //? it's a little op, keep it for now
                (s, o) => {
                    for sindex in self
                        .sectors
                        .data
                        .iter()
                        .filter_map(|x| (x.r#type == *s).then_some(x.index))
                    {
                        if self.sectors.prev(sindex).r#type != *o
                            && self.sectors.next(sindex).r#type != *o
                        {
                            return false;
                        }
                    }
                    true
                }
            },
            ClueConnection::OneAdjacent => match (subject, object) {
                (s, o) if s == o => false,                        // op or useless clue
                (SectorType::Nebula, SectorType::Space) => false, // not very useful
                (s, o) => self
                    .sectors
                    .data
                    .iter()
                    .filter_map(|x| (x.r#type == *s).then_some(x.index))
                    .any(|sindex| {
                        self.sectors.prev(sindex).r#type == *o
                            || self.sectors.next(sindex).r#type == *o
                    }),
            },
            ClueConnection::NotAdjacent => match (subject, object) {
                (s, o) if s == o => false,                         // op or useless clue
                (SectorType::Nebula, SectorType::Space) => false,  // definitely false
                (SectorType::X, SectorType::DwarfPlanet) => false, // useless
                (s, o) => self
                    .sectors
                    .data
                    .iter()
                    .filter_map(|x| (x.r#type == *s).then_some(x.index))
                    .all(|sindex| {
                        self.sectors.prev(sindex).r#type != *o
                            && self.sectors.next(sindex).r#type != *o
                    }),
            },
            ClueConnection::OneOpposite => {
                self.sectors.data.iter().any(|x| {
                    x.r#type == *subject && self.sectors.opposite(x.index).r#type == *object
                })
            }
            ClueConnection::NotOpposite => {
                *subject != *object
                    && self.sectors.data.iter().all(|x| {
                        x.r#type != *subject || self.sectors.opposite(x.index).r#type != *object
                    })
                // if subject == object {
                //     return false;
                // }
                // // or not quite useful if different without X
                // ((*subject != SectorType::X && *subject == *object) || (*subject != SectorType::X))
                //     && self.sectors.data.iter().all(|x| {
                //         x.r#type != *subject || self.sectors.opposite(x.index).r#type != *object
                //     })
            }
            ClueConnection::AllInRange(range) => match (subject, object) {
                (s, o) if s == o => {
                    *s != SectorType::DwarfPlanet // not very useful for dwarf
                        && self.sectors.check_type_max_distance(s) <= *range
                }
                (SectorType::Nebula, SectorType::Space) => false, //not useful
                (s, o) => self
                    .sectors
                    .data
                    .iter()
                    .filter(|&x| x.r#type == *s)
                    .all(|x| self.sectors.check_range_exist(x.index, o, *range)),
            },

            ClueConnection::NotInRange(range) => match (subject, object) {
                (s, o) if s == o => false, // useless
                (s, o) => self
                    .sectors
                    .data
                    .iter()
                    .filter(|&x| x.r#type == *s)
                    .all(|x| !self.sectors.check_range_exist(x.index, o, *range)),
            },
        }
    }
}
fn check_x_space_only(clues: &[Clue], xclues: &[Clue], sectors: &Sectors) -> Vec<usize> {
    // println!("clues: {:?}", xclues);
    let defaults = vec![
        Clue {
            index: ClueEnum::A,
            subject: SectorType::X,
            object: SectorType::DwarfPlanet,
            conn: ClueConnection::NotAdjacent,
        },
        Clue {
            index: ClueEnum::A,
            subject: SectorType::Nebula,
            object: SectorType::Space,
            conn: ClueConnection::AllAdjacent,
        },
    ];
    let all_clues = clues
        .iter()
        .chain(xclues.iter())
        .chain(defaults.iter())
        .collect::<Vec<_>>();
    let possible_x: Vec<_> = sectors
        .data
        .iter()
        .filter(|x| x.r#type == SectorType::Space)
        .filter(|&x| {
            let mut temp_sectors = sectors.clone();
            // swap x with this space
            temp_sectors.data.iter_mut().for_each(|s| {
                if s.r#type == SectorType::X {
                    s.r#type = SectorType::Space;
                }
            });
            temp_sectors.data[x.index - 1].r#type = SectorType::X;

            let check_clue_with_sectors = |clue: &Clue, sectors: &Sectors| match &clue.conn {
                ClueConnection::AllAdjacent => sectors
                    .data
                    .iter()
                    .filter(|s| s.r#type == clue.subject)
                    .all(|s| {
                        sectors.prev(s.index).r#type == clue.object
                            || sectors.next(s.index).r#type == clue.object
                    }),
                ClueConnection::OneAdjacent => sectors
                    .data
                    .iter()
                    .filter(|s| s.r#type == clue.subject)
                    .any(|s| {
                        sectors.prev(s.index).r#type == clue.object
                            || sectors.next(s.index).r#type == clue.object
                    }),
                ClueConnection::NotAdjacent => sectors
                    .data
                    .iter()
                    .filter(|s| s.r#type == clue.subject)
                    .all(|s| {
                        sectors.prev(s.index).r#type != clue.object
                            && sectors.next(s.index).r#type != clue.object
                    }),
                ClueConnection::OneOpposite => sectors.data.iter().any(|s| {
                    s.r#type == clue.subject && sectors.opposite(s.index).r#type == clue.object
                }),
                ClueConnection::NotOpposite => sectors.data.iter().all(|s| {
                    s.r#type != clue.subject || sectors.opposite(s.index).r#type != clue.object
                }),
                ClueConnection::AllInRange(range) => sectors
                    .data
                    .iter()
                    .filter(|s| s.r#type == clue.subject)
                    .all(|s| sectors.check_range_exist(s.index, &clue.object, *range)),
                ClueConnection::NotInRange(range) => sectors
                    .data
                    .iter()
                    .filter(|s| s.r#type == clue.subject)
                    .all(|s| !sectors.check_range_exist(s.index, &clue.object, *range)),
            };

            all_clues
                .iter()
                .all(|clue| check_clue_with_sectors(clue, &temp_sectors))
        })
        .map(|f| f.index)
        .collect();

    // println!(
    //     "clues: {:?}",
    //     clues.iter().map(|x| x.to_string()).collect::<Vec<_>>()
    // );
    // println!(
    //     "xclues: {:?}",
    //     xclues.iter().map(|x| x.to_string()).collect::<Vec<_>>()
    // );
    // println!("possible_x: {:?}", possible_x);
    possible_x
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rand::RngCore;

    use crate::map::{
        Sector,
        model::{Map, MapType},
    };

    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_clue() {
        let mut sum = 0;
        let mut last_failed_seed = 0;
        let mut clue_type_sum = BTreeMap::new();
        for i in 0..=6 {
            clue_type_sum.insert(i, 0);
        }
        for seed in 0..300 {
            dbg!(seed);
            let mut rng = SmallRng::seed_from_u64(seed);
            loop {
                let map = Map::new(rng.clone(), MapType::Expert).unwrap();
                let mut cg = ClueGenerator::new(seed, map.sectors.clone(), map.r#type.clone());
                // for sector in &map.sectors.data {
                // println!("{}", sector);
                // }
                if let Ok((clues, xclues)) = cg.generate_clues() {
                    // println!("clues: {}", clues.len());
                    for clue in clues.iter() {
                        let index = match clue.conn {
                            ClueConnection::AllAdjacent => 0,
                            ClueConnection::OneAdjacent => 1,
                            ClueConnection::NotAdjacent => 2,
                            ClueConnection::OneOpposite => 3,
                            ClueConnection::NotOpposite => 4,
                            ClueConnection::AllInRange(_) => 5,
                            ClueConnection::NotInRange(_) => 6,
                        };
                        let count = clue_type_sum.entry(index).or_insert(0);
                        *count += 1;

                        // if matches!(clue.conn, ClueConnection::AllInRange(_))
                        //     && clue.object == clue.subject
                        // {
                        //     println!("clue: {: <10} {}", clue.as_secret(), clue);
                        //     for sector in &map.sectors.data {
                        //         println!("sector: {}", sector);
                        //     }
                        // }

                        // println!("{: <10}: {}", clue.as_secret(), clue);
                    }
                    // println!("xclues: {}", xclues.len());
                    // for clue in xclues.iter() {
                    //     println!("{: <10}: {}", clue.as_secret(), clue);
                    // }
                    break;
                } else {
                    println!("failed at seed {}", seed);
                    sum += 1;
                    last_failed_seed = seed;
                    rng.next_u32(); // next seed
                }
            }
        }
        println!("failed sum: {}", sum);
        println!("last failed seed: {}", last_failed_seed);
        for (i, count) in clue_type_sum.iter() {
            println!("clue type {}: {}", i, count);
        }
    }

    #[test]
    fn test_check_x_space_only() {
        #[rustfmt::skip]
        let s = Sectors{ data: vec![
            Sector { index: 1, r#type: SectorType::Asteroid },
            Sector { index: 2, r#type: SectorType::X },
            Sector { index: 3, r#type: SectorType::Space },
            Sector { index: 4, r#type: SectorType::Nebula },
            Sector { index: 5, r#type: SectorType::DwarfPlanet },
        ]};
        assert_eq!(
            check_x_space_only(
                &[],
                &[Clue {
                    index: ClueEnum::X1,
                    subject: SectorType::X,
                    object: SectorType::DwarfPlanet,
                    conn: ClueConnection::NotAdjacent
                }],
                &s
            ),
            Vec::<usize>::new()
        );
        #[rustfmt::skip]
        let s = Sectors{ data: vec![
            Sector { index: 1, r#type: SectorType::Asteroid },
            Sector { index: 2, r#type: SectorType::X },
            Sector { index: 3, r#type: SectorType::Space },
            Sector { index: 4, r#type: SectorType::Nebula },
            Sector { index: 5, r#type: SectorType::Space },
            Sector { index: 6, r#type: SectorType::DwarfPlanet },
        ]};
        assert_eq!(
            check_x_space_only(
                &[],
                &[Clue {
                    index: ClueEnum::X1,
                    subject: SectorType::X,
                    object: SectorType::DwarfPlanet,
                    conn: ClueConnection::NotAdjacent
                }],
                &s
            ),
            vec![3]
        );
        #[rustfmt::skip]
        let s = Sectors{ data: vec![
            Sector { index: 1, r#type: SectorType::Asteroid },
            Sector { index: 2, r#type: SectorType::X },
            Sector { index: 3, r#type: SectorType::Nebula },
            Sector { index: 4, r#type: SectorType::Space },
            Sector { index: 5, r#type: SectorType::Space },
            Sector { index: 6, r#type: SectorType::DwarfPlanet },
        ]};
        assert_eq!(
            check_x_space_only(
                &[],
                &[Clue {
                    index: ClueEnum::X1,
                    subject: SectorType::X,
                    object: SectorType::DwarfPlanet,
                    conn: ClueConnection::NotAdjacent
                }],
                &s
            ),
            vec![4]
        );
        #[rustfmt::skip]
        let s = Sectors{ data: vec![
            Sector { index: 1, r#type: SectorType::Space },
            Sector { index: 2, r#type: SectorType::X },
            Sector { index: 3, r#type: SectorType::Nebula },
            Sector { index: 4, r#type: SectorType::Space },
            Sector { index: 5, r#type: SectorType::Asteroid },
            Sector { index: 6, r#type: SectorType::Asteroid },
        ]};
        assert_eq!(
            check_x_space_only(
                &[Clue {
                    index: ClueEnum::A,
                    subject: SectorType::Asteroid,
                    object: SectorType::Space,
                    conn: ClueConnection::AllAdjacent
                }],
                &[Clue {
                    index: ClueEnum::X1,
                    subject: SectorType::X,
                    object: SectorType::DwarfPlanet,
                    conn: ClueConnection::NotAdjacent
                }],
                &s
            ),
            Vec::<usize>::new()
        );
    }
}
