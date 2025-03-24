use core::panic;

use rand::{Rng, SeedableRng, rngs::SmallRng};
use serde::Serialize;

use super::model::{SectorType, Sectors};

#[derive(Debug, Clone)]
pub struct Clue {
    pub subject: SectorType,
    pub object: SectorType,
    pub conn: ClueConnection,
}

impl Serialize for Clue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self)
    }
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
            ClueConnection::AllInRange(n) => match &self.object == &self.subject {
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
        return format!("{} {}", self.subject, self.object);
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
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
}

impl ClueGenerator {
    pub fn new(seed: u64, sectors: Sectors) -> Self {
        Self {
            seed,
            rng: SmallRng::seed_from_u64(seed),
            sectors,
        }
    }

    pub fn generate_clues(&mut self) -> anyhow::Result<(Vec<Clue>, Vec<Clue>)> {
        let mut res = vec![];
        while res.len() < 6 {
            let subject = self.get_rand_type(false, false);
            let object = self.get_rand_type(true, false);

            let conn = self.get_rand_conn(false);
            if !self.check_clue(&res, &subject, &object, &conn) {
                continue;
            }
            res.push(Clue {
                subject,
                object,
                conn,
            });
        }
        let mut xres = vec![];
        let mut cnt = 0;
        while !check_x_space_only(&xres, &self.sectors) {
            xres.clear();
            while xres.len() < 2 {
                let subject = SectorType::X;
                let object = self.get_rand_type(true, false);

                let conn = self.get_rand_conn(true);
                if !self.check_clue(&xres, &subject, &object, &conn) {
                    continue;
                }
                xres.push(Clue {
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
        match self.rng.random_range(0..=6) {
            0 => ClueConnection::AllAdjacent,
            1 => ClueConnection::OneAdjacent,
            2 => ClueConnection::NotAdjacent,
            3 => ClueConnection::OneOpposite,
            4 => ClueConnection::NotOpposite,
            5 => {
                ClueConnection::AllInRange(self.rng.random_range(if is_x { 2..=4 } else { 4..=6 }))
            }
            _ => {
                ClueConnection::NotInRange(self.rng.random_range(if is_x { 3..=4 } else { 2..=3 }))
            }
        }
    }

    fn check_clue(
        &self,
        clues: &[Clue],
        subject: &SectorType,
        object: &SectorType,
        conn: &ClueConnection,
    ) -> bool {
        for clue in clues {
            // same clue secret
            let try_secret = if object == subject || *object == SectorType::Space {
                format!("{}", subject)
            } else {
                format!("{} {}", subject, object)
            };
            if clue.as_secret() == try_secret {
                return false;
            }

            // same clue pair
            if clue.subject == *subject && clue.object == *object {
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
                (SectorType::Comet, SectorType::Comet) => return false, // op clue show commets are 2 && 3
                (SectorType::Asteroid, SectorType::Asteroid)
                | (SectorType::Nebula, SectorType::Space) => return false, // not very useful
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
                    return true;
                }
            },
            ClueConnection::OneAdjacent => match (subject, object) {
                (s, o) if s == o => return false, // op or useless clue
                (SectorType::Nebula, SectorType::Space) => return false, // not very useful
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
                (s, o) if s == o => return false, // op or useless clue
                (SectorType::Nebula, SectorType::Space) => return false, // definitely false
                (SectorType::X, SectorType::DwarfPlanet) => return false, // useless
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
                (SectorType::Nebula, SectorType::Space) => return false, //not useful
                (s, o) => self
                    .sectors
                    .data
                    .iter()
                    .filter(|&x| x.r#type == *s)
                    .all(|x| self.sectors.check_range_exist(x.index, o, *range)),
            },

            ClueConnection::NotInRange(range) => match (subject, object) {
                (s, o) if s == o => return false, // useless
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
fn check_x_space_only(xclues: &[Clue], sectors: &Sectors) -> bool {
    if xclues.len() == 0 {
        return false;
    }
    // println!("clues: {:?}", xclues);
    let possible_x: Vec<_> = sectors
        .data
        .iter()
        .filter(|x| x.r#type == SectorType::Space)
        .filter(|&x| {
            !(sectors.next(x.index).r#type != SectorType::DwarfPlanet
                && sectors.prev(x.index).r#type != SectorType::DwarfPlanet)
        })
        .filter(|&x| {
            let next = sectors.next(x.index);
            let pre = sectors.prev(x.index);
            !((next.r#type == SectorType::Nebula
                && sectors.next(next.index).r#type != SectorType::Space
                && sectors.next(next.index).r#type != SectorType::X)
                || (pre.r#type == SectorType::Nebula
                    && sectors.prev(pre.index).r#type != SectorType::Space
                    && sectors.prev(pre.index).r#type != SectorType::X))
        })
        .filter(|&x| {
            let mut temp_sectors = sectors.clone();
            // swap x with this space
            temp_sectors.data.iter_mut().for_each(|s| {
                if s.r#type == SectorType::X {
                    s.r#type = SectorType::Space;
                }
            });
            temp_sectors.data[x.index - 1].r#type = SectorType::X;
            // println!("temp_sectors:");
            // for t in temp_sectors.data.iter() {
            //     println!("{}", t);
            // }

            !xclues.iter().all(|clue| match (&clue.conn, &clue.object) {
                (ClueConnection::AllAdjacent, o) | (ClueConnection::OneAdjacent, o) => {
                    temp_sectors.next(x.index).r#type == *o
                        || temp_sectors.prev(x.index).r#type == *o
                }
                (ClueConnection::NotAdjacent, o) => {
                    temp_sectors.next(x.index).r#type != *o
                        && temp_sectors.prev(x.index).r#type != *o
                }
                (ClueConnection::OneOpposite, o) => temp_sectors.opposite(x.index).r#type == *o,
                (ClueConnection::NotOpposite, o) => temp_sectors.opposite(x.index).r#type != *o,
                (ClueConnection::AllInRange(range), o) => {
                    temp_sectors.check_range_exist(x.index, o, *range)
                }
                (ClueConnection::NotInRange(range), o) => {
                    !temp_sectors.check_range_exist(x.index, o, *range)
                }
            })
        })
        .collect();

    // println!("possible_x: {:?}", possible_x);
    return possible_x.len() == 0;
}

#[cfg(test)]
mod tests {
    use rand::RngCore;

    use crate::map::model::{Map, MapType};

    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_clue() {
        let mut sum = 0;
        let mut last_failed_seed = 0;
        for seed in 1..1000 {
            dbg!(seed);
            let mut rng = SmallRng::seed_from_u64(seed);
            loop {
                let map = Map::new(rng.clone(), MapType::Standard).unwrap();
                // dbg!(&map.sectors.data);
                let mut cg = ClueGenerator::new(seed, map.sectors.clone());
                for sector in &map.sectors.data {
                    println!("{}", sector);
                }
                if let Ok((clues, xclues)) = cg.generate_clues() {
                    println!("clues:");
                    for clue in clues.iter() {
                        // match clue.conn {
                        //     ClueConnection::NotInRange(_) => {
                        //         println!("{: <10}: {}", clue.as_secret(), clue);
                        //     }
                        //     _ => (),
                        // }
                        println!("{: <10}: {}", clue.as_secret(), clue);
                    }
                    println!("xclues:");
                    for clue in xclues.iter() {
                        println!("{: <10}: {}", clue.as_secret(), clue);
                    }
                    break;
                } else {
                    println!("failed at seed {}", seed);
                    sum += 1;
                    last_failed_seed = seed;
                    rng.next_u32(); // next seed
                }
            }
            // dbg!(clues);
        }
        println!("failed sum: {}", sum);
        println!("last failed seed: {}", last_failed_seed);
    }
}
