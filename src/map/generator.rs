use std::collections::HashMap;

use rand::{rngs::SmallRng, Rng, SeedableRng};

use super::model::{MapType, Sector, SectorType};

pub struct MapGenerator {
    seed: u64,
    rng: SmallRng,
    map_type: MapType,

    rest_index: Vec<u32>,
    temp: HashMap<SectorType, Vec<Sector>>,
}

impl MapGenerator {
    pub fn new(seed: u64, map_type: &MapType) -> Self {
        let rng = SmallRng::seed_from_u64(seed);
        let rest_index = (1..=match map_type {
            MapType::Beginner => 12,
            MapType::Master => 18,
        })
            .collect();
        Self {
            seed,
            rng,
            map_type: map_type.clone(),
            rest_index,
            temp: HashMap::new(),
        }
    }

    /// rules for generating sectors.
    /// 1. The number of sectors of each type is fixed.
    /// 2. The order of sectors is random but the same seed should generate the same map.
    /// 3. The index of sectors is 1-based.
    /// 4. Comet only appears in prime index sectors. like 2, 3, 5, 7, 11, 13, 17.
    /// 5. Asteroid ALWAYS pairs with another Asteroid. So is might be a,a...a,a. or a,a,a,a.
    /// 6. DwarfPlanet MUST not be adjacent to X.
    /// 7. In Master map, there are 4 DwarfPlanet sectors, and they are ALWAYS be in the contiguous 6 sectors.
    ///    And the beginning and ending of the 6 sectors MUST be DwarfPlanet.
    ///    So the possible combinations are: d??ddd, d?d?dd, d?dd?d, dd??dd, dd?d?d, ddd??d.
    /// 8. Nebula will ALWAYS be adjacent to at least one Empty sector.
    pub fn generate_sectors(&mut self) -> anyhow::Result<Vec<Sector>> {
        let sector_types = match self.map_type {
            MapType::Beginner => BEGINNER_TYPES,
            MapType::Master => MASTER_TYPES,
        };

        while self.temp.len() < sector_types.len() {
            let mut ok = false;
            let sector_type = &sector_types[self.temp.len()].0;
            let count = sector_types[self.temp.len()].1;
            let test_count = if *sector_type == SectorType::X {
                1
            } else {
                RAND_TRY_TIMES
            };
            for _i in 0..test_count {
                if let Ok(generated_sectors) = self.generate_sectors_by_type(sector_type, count) {
                    // println!(
                    //     "Generated {sector_type:?} at {:?}",
                    //     generated_sectors
                    //         .iter()
                    //         .map(|s| s.index)
                    //         .collect::<Vec<_>>()
                    // );
                    self.temp.insert(sector_type.clone(), generated_sectors);
                    ok = true;
                    break;
                }
            }
            if !ok {
                if *sector_type == SectorType::X {
                    println!("Failed to generate X, retry from the beginning.");
                    self.temp.clear();
                    self.rest_index = (1..=match self.map_type {
                        MapType::Beginner => 12,
                        MapType::Master => 18,
                    })
                        .collect();
                } else {
                    let prev = &sector_types[self.temp.len() - 1].0;
                    println!("Failed to generate {:?}, try again back", sector_type);
                    println!("current rest index: {:?}", self.rest_index);
                    println!("prev: {:?}", prev);

                    if let Some(s) = self.temp.remove(prev) {
                        self.rest_index.extend(s.iter().map(|s| s.index));
                        // self.rest_index.sort();
                    }
                    println!("back rest index: {:?}", self.rest_index);
                }
                // assert!(false);
            }
        }

        let mut results: Vec<Sector> = self.temp.drain().map(|(_, v)| v).flatten().collect();
        results.sort_by_key(|s| s.index);
        Ok(results)
    }

    fn generate_sectors_by_type(
        &mut self,
        r#type: &SectorType,
        count: u32,
    ) -> anyhow::Result<Vec<Sector>> {
        // println!("Generating {:?} * {}", r#type, count);
        let mut sectors = vec![];
        for _i in 0..count {
            if let Ok(sector) = match r#type {
                SectorType::Comet => self.generate_comet_sector(),
                SectorType::Asteroid => self.generate_asteroid_sector(),
                SectorType::DwarfPlanet => self.generate_dwarf_planet_sector(),
                SectorType::Nebula => {
                    assert!(count <= 2); // only support 1 or 2 for now. Stupid limit generate method as nebula can take the Space at second time. no more.
                    self.generate_nebula_sector(sectors.last().map(|s: &Sector| s.index))
                }
                SectorType::X => self.generate_x_sector(),
                SectorType::Space => self.generate_space_sector(),
            } {
                // println!(
                //     "Generated result {:?} at {:?}",
                //     r#type,
                //     sector.iter().map(|s| s.index).collect::<Vec<_>>()
                // );
                sectors.extend(sector);
            } else {
                self.rest_index
                    .append(&mut sectors.iter().map(|s| s.index).collect::<Vec<_>>());
                return Err(anyhow::anyhow!("Failed to generate sector."));
            }
        }
        Ok(sectors)
    }

    fn generate_asteroid_sector(&mut self) -> anyhow::Result<Vec<Sector>> {
        let (left, right) = self.get_rest_pair_index()?;
        Ok(vec![
            Sector {
                index: left,
                r#type: SectorType::Asteroid,
            },
            Sector {
                index: right,
                r#type: SectorType::Asteroid,
            },
        ])
    }

    fn generate_comet_sector(&mut self) -> anyhow::Result<Vec<Sector>> {
        let index = self.get_rand_rest_prime_index()?;
        Ok(vec![Sector {
            index,
            r#type: SectorType::Comet,
        }])
    }

    fn generate_dwarf_planet_sector(&mut self) -> anyhow::Result<Vec<Sector>> {
        match self.map_type {
            MapType::Beginner => {
                let index = self.get_rest_index()?;
                Ok(vec![Sector {
                    index,
                    r#type: SectorType::DwarfPlanet,
                }])
            }
            MapType::Master => {
                // get a rand 6 sectors with at least 4 rooms for DwarfPlanet.
                let range = self.get_rand_range(6, 4)?;
                Ok(range
                    .iter()
                    .map(|&index| Sector {
                        index,
                        r#type: SectorType::DwarfPlanet,
                    })
                    .collect())
            }
        }
    }

    fn generate_nebula_sector(
        &mut self,
        current_nebula_index: Option<u32>,
    ) -> anyhow::Result<Vec<Sector>> {
        for _ in 0..RAND_TRY_TIMES {
            let (result, others) = self.try_get_pair_index()?;
            self.rest_index.push(others);
            if let Some(nebula_index) = current_nebula_index {
                let (left, right) = self.adjacent_index(nebula_index);
                if left == result && !self.rest_index.contains(&right) {
                    self.rest_index.push(result);
                    continue;
                }
                if right == result && !self.rest_index.contains(&left) {
                    self.rest_index.push(result);
                    continue;
                }
            }
            return Ok(vec![Sector {
                index: result,
                r#type: SectorType::Nebula,
            }]);
        }
        Err(anyhow::anyhow!("Failed to generate Nebula sector."))

        // Ok(vec![Sector {
        //     index: result,
        //     r#type: SectorType::Nebula,
        // }])
    }

    fn generate_space_sector(&mut self) -> anyhow::Result<Vec<Sector>> {
        let index = self.get_rest_index()?;
        Ok(vec![Sector {
            index,
            r#type: SectorType::Space,
        }])
    }

    fn generate_x_sector(&mut self) -> anyhow::Result<Vec<Sector>> {
        // the X sector must not be adjacent to DwarfPlanet.
        // and the X can not take the Space sector adjacent to Nebula.

        for index in self.rest_index.clone() {
            // get the current dwarf planet sector index.
            let dwarf_planet_index = self
                .temp
                .get(&SectorType::DwarfPlanet)
                .map(|s| s.iter().map(|s| s.index).collect::<Vec<_>>())
                .unwrap_or_default();
            let (left, right) = self.adjacent_index(index);
            if dwarf_planet_index.contains(&left) || dwarf_planet_index.contains(&right) {
                continue;
            }

            // get the current nebula sector index.
            let nebula_index = self
                .temp
                .get(&SectorType::Nebula)
                .map(|s| s.iter().map(|s| s.index).collect::<Vec<_>>())
                .unwrap_or_default();
            // println!("nebula_index: {:?}", nebula_index);
            // println!("rest_index: {:?}", self.rest_index);
            // println!("try index: {:?}", index);
            // check the rest rest_index is valid Space sector adjacent to Nebula.
            let rest_rest_index = self
                .rest_index
                .iter()
                .filter(|&x| *x != index)
                .cloned()
                .collect::<Vec<_>>();
            let mut valid = true;
            for nebul in nebula_index {
                let (left, right) = self.adjacent_index(nebul);
                if !rest_rest_index.contains(&left) && !rest_rest_index.contains(&right) {
                    valid = false;
                    break;
                }
            }
            if !valid {
                continue;
            }
            self.rest_index.retain(|&x| x != index);
            return Ok(vec![Sector {
                index,
                r#type: SectorType::X,
            }]);
        }
        Err(anyhow::anyhow!("Failed to generate X sector."))
    }

    // helper functions below.

    fn get_rand_rest_prime_index(&mut self) -> anyhow::Result<u32> {
        for _ in 0..RAND_TRY_TIMES {
            let index = self.rng.random_range(1..=self.map_type.sector_count());
            if is_prime(index) && self.rest_index.contains(&index) {
                self.rest_index.retain(|&x| x != index);
                return Ok(index);
            }
        }
        Err(anyhow::anyhow!("Failed to get prime index."))
    }

    fn get_rest_index(&mut self) -> anyhow::Result<u32> {
        if self.rest_index.is_empty() {
            return Err(anyhow::anyhow!("No more rest index."));
        }
        let index = self.rng.random_range(0..self.rest_index.len());
        Ok(self.rest_index.remove(index))
    }

    fn get_rest_pair_index(&mut self) -> anyhow::Result<(u32, u32)> {
        for _ in 0..RAND_TRY_TIMES {
            if let Ok(pair) = self.try_get_pair_index() {
                return Ok(pair);
            }
        }
        Err(anyhow::anyhow!("Failed to get pair index."))
    }

    fn get_rand_range(&mut self, range_len: u32, empty_len: u32) -> anyhow::Result<Vec<u32>> {
        for _ in 0..RAND_TRY_TIMES {
            let st = self.rest_index[self.rng.random_range(0..self.rest_index.len())];
            let ed = (st + range_len - 1) % self.map_type.sector_count();
            // println!("st: {}, ed: {}", st, ed);
            if !self.rest_index.contains(&ed) {
                continue;
            }
            let mut range_indexs = vec![];
            for i in 1..range_len - 1 {
                let index = (st + i - 1) % self.map_type.sector_count() + 1;
                if !self.rest_index.contains(&index) {
                    continue;
                }
                range_indexs.push(index);
            }
            // println!("range_indexs: {:?}", range_indexs);

            if range_indexs.len() < (empty_len - 2) as usize {
                continue;
            }

            let mut res = vec![st, ed];
            while res.len() < empty_len as usize {
                let index = range_indexs[self.rng.random_range(0..range_indexs.len())];
                range_indexs.retain(|&x| x != index);
                res.push(index);
            }

            // println!("res: {:?}", res);
            for i in 0..res.len() {
                self.rest_index.retain(|&x| x != res[i]);
            }
            // sort the result make sure the first and last is st and ed.
            res.sort_by_key(|&x| {
                (x + self.map_type.sector_count() - st) % self.map_type.sector_count()
            });

            // println!("res: {:?}", res);

            return Ok(res);
        }
        Err(anyhow::anyhow!("Failed to get rand range."))
    }

    fn get_rand_bool(&mut self) -> bool {
        self.rng.random_bool(0.5)
    }

    fn try_get_pair_index(&mut self) -> anyhow::Result<(u32, u32)> {
        let index = self.get_rest_index()?;
        let (left, right) = self.adjacent_index(index);

        let (left, right) = if self.get_rand_bool() {
            (left, right)
        } else {
            (right, left)
        };

        if self.rest_index.contains(&left) {
            self.rest_index.retain(|&x| x != left);
            return Ok((left, index));
        } else if self.rest_index.contains(&right) {
            self.rest_index.retain(|&x| x != right);
            return Ok((index, right));
        }
        self.rest_index.push(index);
        Err(anyhow::anyhow!("Failed to get pair index."))
    }

    fn adjacent_index(&mut self, from: u32) -> (u32, u32) {
        assert!(from > 0 && from <= self.map_type.sector_count());
        let max = self.map_type.sector_count();

        let left = if from == 1 { max } else { from - 1 };
        let right = if from == max { 1 } else { from + 1 };

        (left, right)
    }

    /// debug function.
    fn check_sectors_rules(&mut self, sectors: &[Sector]) -> bool {
        for Sector { index, r#type } in sectors {
            let (left, right) = self.adjacent_index(*index);
            let left_sector = sectors.iter().find(|s| s.index == left).unwrap();
            let right_sector = sectors.iter().find(|s| s.index == right).unwrap();
            match r#type {
                SectorType::Comet => {
                    if !is_prime(*index) {
                        return false;
                    }
                }
                SectorType::Asteroid => {
                    if left_sector.r#type != SectorType::Asteroid
                        && right_sector.r#type != SectorType::Asteroid
                    {
                        println!("Asteroid adjacent error.");
                        assert!(false);
                        return false;
                    }
                }
                SectorType::DwarfPlanet => {
                    if left_sector.r#type == SectorType::X || right_sector.r#type == SectorType::X {
                        println!("DwarfPlanet adjacent to X error.");
                        assert!(false);
                        return false;
                    }
                }
                SectorType::Nebula => {
                    if left_sector.r#type != SectorType::Space
                        && right_sector.r#type != SectorType::Space
                    {
                        println!("Nebula adjacent to Space error.");
                        assert!(false);
                        return false;
                    }
                }
                SectorType::X => {
                    if left_sector.r#type == SectorType::DwarfPlanet
                        || right_sector.r#type == SectorType::DwarfPlanet
                    {
                        println!("X adjacent to DwarfPlanet error.");
                        assert!(false);
                        return false;
                    }
                }
                SectorType::Space => {}
            }
        }
        true
    }
}

const RAND_TRY_TIMES: u32 = 10;

fn is_prime(n: u32) -> bool {
    // actually, we only need to check if n is a prime number less than 18.
    // so we can just hard code the prime numbers.
    match n {
        2 | 3 | 5 | 7 | 11 | 13 | 17 => true,
        _ => false,
    }
}

// 12 sectors. 2 + 4 + 1 + 2 + 1 + 2 = 12
const BEGINNER_TYPES: [(SectorType, u32); 6] = [
    (SectorType::Comet, 2),
    (SectorType::Asteroid, 2), // 2*2 = 4
    (SectorType::DwarfPlanet, 1),
    (SectorType::Nebula, 2),
    (SectorType::X, 1),
    (SectorType::Space, 2),
];

// 18 sectors. 2 + 4 + 4 + 2 + 1 + 5 = 18
const MASTER_TYPES: [(SectorType, u32); 6] = [
    (SectorType::Comet, 2),
    (SectorType::Asteroid, 2),    // 2*2 = 4
    (SectorType::DwarfPlanet, 1), // 1*4 = 4
    (SectorType::Nebula, 2),
    (SectorType::X, 1),
    (SectorType::Space, 5),
];

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_generator() {
        for seed in 0..100000 {
            println!("Seed: {}", seed);
            let mut g = MapGenerator::new(seed, &MapType::Master);
            let r = g.generate_sectors();
            // println!("{:#?}", r);
            assert!(r.is_ok());
            let r = r.unwrap();
            assert!(g.check_sectors_rules(&r));
        }
        // let mut g = MapGenerator::new(96, &MapType::Master);
        // let r = g.generate_sectors();
        // println!("{:#?}", r);
        // println!("{:?}", g.get_rand_rest_prime_index());
        // println!("{:?}", g.get_rand_rest_prime_index());

        // println!("{:?}", g.get_rest_index());
        // println!("{:?}", g.get_rest_index());
        // println!("{:?}", g.get_rest_index());
        // println!("{:?}", g.get_rest_index());
        // dbg!(g.rest_index.clone());
        // println!("{:?}", g.get_rand_range(6, 4));
        // dbg!(g.rest_index);
        // println!("{:?}", g.get_rest_index());

        // println!("{:?}", g.get_rest_pair_index());
        // println!("{:?}", g.get_rest_pair_index());
        // println!("{:?}", g.rest_index);

        // println!("{:?}", g.adjacent_index(1));
        // println!("{:?}", g.adjacent_index(2));
        // println!("{:?}", g.adjacent_index(6));
        // println!("{:?}", g.adjacent_index(17));
        // println!("{:?}", g.adjacent_index(18));
    }
}
