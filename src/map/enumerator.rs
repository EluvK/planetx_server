use itertools::Itertools;
use std::collections::{HashMap, HashSet};

use super::{MapType, Sector, SectorType, Sectors};

type Position = usize;

const PRIMES_EXPERT: [Position; 7] = [1, 2, 4, 6, 10, 12, 16]; // 0-based positions for 2,3,5,7,11,13,17
const PRIMES_STANDARD: [Position; 5] = [1, 2, 4, 6, 10]; // 0-based positions for 2,3,5,7,11

pub struct MapEnumerator {
    predef_d_e_standard: HashMap<Vec<Position>, Vec<([Position; 2], Vec<Position>)>>,
    predef_d_e_expert: HashMap<Vec<Position>, Vec<([Position; 2], Vec<Position>)>>,
}

// a: Comet, b: Asteroid, c: DwarfPlanet, d: Nebula, e: Space, f: X
impl MapEnumerator {
    pub fn new() -> Self {
        let predef_d_e_standard = pre_generate_d_e_standard().collect::<HashMap<_, _>>();
        let predef_d_e_expert = pre_generate_d_e_expert().collect::<HashMap<_, _>>();
        Self {
            predef_d_e_standard,
            predef_d_e_expert,
        }
    }

    pub fn gen_sec(&self, map_type: &MapType) -> impl Iterator<Item = Sectors> {
        generate_c(map_type).flat_map(move |c| {
            generate_f(&c, map_type).flat_map(move |f| {
                generate_a(&c, f, map_type).flat_map({
                    let c = c.clone();
                    move |a| {
                        generate_b(&c, f, &a, map_type)
                            .filter_map({
                                let c = c.clone();
                                move |b| {
                                    let pos: Vec<_> = (0..map_type.sector_count())
                                        .filter(|p| {
                                            !a.contains(p)
                                                && !b.contains(p)
                                                && !c.contains(p)
                                                && *p != f
                                        })
                                        .collect();
                                    let c = c.clone();
                                    // println!("pos: {:?}", pos);
                                    match map_type {
                                        MapType::Standard => {
                                            self.predef_d_e_standard.get(&pos).map(|de| {
                                                de.iter()
                                                    .map(move |(d, e)| {
                                                        build_sectors(&c, f, &a, &b, d, e)
                                                    })
                                                    .collect::<Vec<_>>()
                                            })
                                        }
                                        MapType::Expert => {
                                            self.predef_d_e_expert.get(&pos).map(|de| {
                                                de.iter()
                                                    .map(move |(d, e)| {
                                                        build_sectors(&c, f, &a, &b, d, e)
                                                    })
                                                    .collect::<Vec<_>>()
                                            })
                                        }
                                    }
                                }
                            })
                            .flatten()
                    }
                })
            })
        })
    }
}

fn generate_c(map_type: &MapType) -> Box<dyn Iterator<Item = Vec<Position>>> {
    let cnt = map_type.sector_count();
    match map_type {
        MapType::Standard => Box::new((0..cnt).map(|i| vec![i])),
        MapType::Expert => Box::new((0..cnt).flat_map(move |start| {
            let end = (start + 5) % cnt;
            let mids = [
                (start + 1) % cnt,
                (start + 2) % cnt,
                (start + 3) % cnt,
                (start + 4) % cnt,
            ];
            mids.iter()
                .combinations(2)
                .map(move |comb| vec![start, end, *comb[0], *comb[1]])
                .collect::<Vec<_>>()
        })),
    }
}

fn generate_f(c: &[Position], map_type: &MapType) -> Box<dyn Iterator<Item = Position>> {
    // 使用位图或数组代替HashSet，因为Position范围小(0-17)
    let cnt = map_type.sector_count();
    let mut excluded = vec![false; cnt];
    for &pos in c {
        excluded[(pos + cnt - 1) % cnt] = true;
        excluded[(pos + 1) % cnt] = true;
    }

    let available: Vec<_> = (0..cnt)
        .filter(|&p| !c.contains(&p) && !excluded[p])
        .collect();

    Box::new(available.into_iter())
}

fn generate_a(
    c: &[Position],
    f: Position,
    map_type: &MapType,
) -> Box<dyn Iterator<Item = [Position; 2]>> {
    let available = match map_type {
        MapType::Standard => PRIMES_STANDARD
            .iter()
            .filter(|&&p| !c.contains(&p) && p != f)
            .cloned()
            .collect::<Vec<_>>(),
        MapType::Expert => PRIMES_EXPERT
            .iter()
            .filter(|&&p| !c.contains(&p) && p != f)
            .cloned()
            .collect::<Vec<_>>(),
    };
    // 直接生成组合而不是先收集再组合
    Box::new(
        available
            .into_iter()
            .tuple_combinations()
            .map(|(a1, a2)| [a1, a2]),
    )
}

fn generate_b(
    c: &[Position],
    f: Position,
    a: &[Position; 2],
    map_type: &MapType,
) -> Box<dyn Iterator<Item = Vec<Position>>> {
    let cnt = map_type.sector_count();
    let available: Vec<Position> = (0..cnt)
        .filter(|p| !c.contains(p) && !a.contains(p) && *p != f)
        .collect();

    // 生成所有4元素组合并过滤
    Box::new(
        available
            .into_iter()
            .combinations(4)
            .filter(move |bs| {
                // 检查每个B位置是否至少有一个相邻B
                bs.iter().all(|&b| {
                    let prev = (b + cnt - 1) % cnt; // 左邻
                    let next = (b + 1) % cnt; // 右邻
                    bs.contains(&prev) || bs.contains(&next)
                })
            })
            // 可选：标准化顺序
            .map(|mut v| {
                v.sort();
                v
            }),
    )
}

fn pre_generate_d_e_standard()
-> Box<dyn Iterator<Item = (Vec<Position>, Vec<([Position; 2], Vec<Position>)>)>> {
    let available: Vec<Position> = (0..12).collect();
    fn neighbors(p: Position) -> [Position; 2] {
        [(p + 12 - 1) % 12, (p + 1) % 12]
    }
    Box::new(available.into_iter().combinations(4).map(|cb| {
        let res = cb
            .clone()
            .into_iter()
            .combinations(2)
            .filter_map(|d| {
                let e: Vec<_> = cb.iter().filter(|p| !d.contains(p)).cloned().collect();
                if e.len() != 2 {
                    assert_eq!(e.len(), 2);
                    return None;
                }

                let d1 = d[0];
                let d2 = d[1];
                let e_set: HashSet<_> = e.iter().cloned().collect();

                let valid_d1 = neighbors(d1).iter().any(|n| e_set.contains(n));
                let valid_d2 = neighbors(d2).iter().any(|n| e_set.contains(n));

                if valid_d1 && valid_d2 {
                    Some(([d1, d2], e))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        (cb, res)
    }))
}

fn pre_generate_d_e_expert()
-> Box<dyn Iterator<Item = (Vec<Position>, Vec<([Position; 2], Vec<Position>)>)>> {
    let available: Vec<Position> = (0..18).collect();
    fn neighbors(p: Position) -> [Position; 2] {
        [(p + 18 - 1) % 18, (p + 1) % 18]
    }
    Box::new(available.into_iter().combinations(7).map(|cb| {
        let res = cb
            .clone()
            .into_iter()
            .combinations(2)
            .filter_map(|d| {
                let e: Vec<_> = cb.iter().filter(|p| !d.contains(p)).cloned().collect();
                if e.len() != 5 {
                    assert_eq!(e.len(), 5);
                    return None;
                }

                let d1 = d[0];
                let d2 = d[1];
                let e_set: HashSet<_> = e.iter().cloned().collect();

                let valid_d1 = neighbors(d1).iter().any(|n| e_set.contains(n));
                let valid_d2 = neighbors(d2).iter().any(|n| e_set.contains(n));

                if valid_d1 && valid_d2 {
                    Some(([d1, d2], e))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        (cb, res)
    }))
}

fn build_sectors(
    c: &[Position],
    f: Position,
    a: &[Position; 2],
    b: &[Position],
    d: &[Position; 2],
    e: &[Position],
) -> Sectors {
    let mut res = Vec::new();
    for a in a {
        res.push(Sector {
            index: *a + 1,
            r#type: SectorType::Comet,
        });
    }
    for b in b {
        res.push(Sector {
            index: *b + 1,
            r#type: SectorType::Asteroid,
        });
    }
    for c in c {
        res.push(Sector {
            index: *c + 1,
            r#type: SectorType::DwarfPlanet,
        });
    }
    for d in d {
        res.push(Sector {
            index: *d + 1,
            r#type: SectorType::Nebula,
        });
    }
    for e in e {
        res.push(Sector {
            index: *e + 1,
            r#type: SectorType::Space,
        });
    }
    res.push(Sector {
        index: f + 1,
        r#type: SectorType::X,
    });
    res.sort_by(|a, b| a.index.cmp(&b.index));
    Sectors { data: res }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn pre() {
        let g = MapEnumerator::new();
        let st = std::time::Instant::now();
        let r1 = g.gen_sec(&MapType::Expert).count();
        let elapsed = st.elapsed();
        println!("count: {}", r1);
        println!("Elapsed time: {:?}", elapsed);

        // let all = g.gen_sec(&MapType::Expert).collect::<Vec<_>>();
        // let mem = std::mem::size_of::<Vec<Sector>>() * all.len()
        //     + std::mem::size_of::<Sector>() * all.iter().map(|v| v.data.len()).sum::<usize>();
        // println!("Memory usage: {} bytes", mem);
        // println!("Memory usage: {} MB", mem as f64 / (1024.0 * 1024.0));

        let st = std::time::Instant::now();
        let r2 = g.gen_sec(&MapType::Standard).count();
        let elapsed = st.elapsed();
        println!("count: {}", r2);
        println!("Elapsed time: {:?}", elapsed);

        g.gen_sec(&MapType::Expert)
            .skip(1123456)
            .take(1)
            .for_each(|v| {
                println!("{:?}", v);
            });
    }
}
