use rand::{distr::StandardUniform, rngs::SmallRng};
use serde::{Deserialize, Serialize};

use super::generator::MapGenerator;

pub struct Map {
    pub r#type: MapType,
    // pub sectors: Vec<Sector>,
    pub sectors: Sectors,
}

#[derive(Clone, Debug)]
pub struct Sectors {
    pub data: Vec<Sector>,
}

impl Sectors {
    pub fn next(&self, index: usize) -> &Sector {
        let next_index = if index == self.data.len() {
            1
        } else {
            index + 1
        };
        &self.data[next_index - 1]
    }
    pub fn prev(&self, index: usize) -> &Sector {
        let prev_index = if index == 1 {
            self.data.len()
        } else {
            index - 1
        };
        &self.data[prev_index - 1]
    }
    pub fn opposite(&self, index: usize) -> &Sector {
        let opposite_index = if index <= self.data.len() / 2 {
            index + self.data.len() / 2
        } else {
            index - self.data.len() / 2
        };
        &self.data[opposite_index - 1]
    }
    pub fn check_range_exist(&self, index: usize, object: &SectorType, range: usize) -> bool {
        // println!(
        //     "check_range_exist: index: {}, object: {:?}, range: {}",
        //     index, object, range
        // );
        let mut nindex = index;
        for _ in 1..=range {
            let next = self.next(nindex);
            if next.r#type == *object {
                return true;
            }
            nindex = next.index;
        }
        let mut pindex = index;
        for _ in 1..=range {
            let prev = self.prev(pindex);
            if prev.r#type == *object {
                return true;
            }
            pindex = prev.index;
        }
        return false;
    }
    pub fn check_type_max_distance(&self, object: &SectorType) -> usize {
        self.data
            .iter()
            .filter(|a| a.r#type == *object) // 筛选出类型匹配的元素
            .flat_map(|a| {
                self.data
                    .iter()
                    .filter(|b| b.r#type == *object) // 再次筛选类型匹配的元素
                    .map(move |b| {
                        let distance = (a.index as isize - b.index as isize).abs() as usize;
                        let wrapped_distance = self.data.len() - distance;
                        distance.min(wrapped_distance) + 1 // 计算最小距离
                    })
            })
            .max() // 找到最大距离
            .unwrap_or(0) // 如果没有匹配项，返回 0
    }
}

#[derive(Clone, Debug)]
pub struct Sector {
    pub index: usize, // 1-based index.
    pub r#type: SectorType,
}

impl std::fmt::Display for Sector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sector {} - {}", self.index, self.r#type)
    }
}

#[derive(Clone)]
pub enum MapType {
    Beginner, // 12 secotrs.
    Master,   // 18 sectors.
}

impl MapType {
    pub const fn sector_count(&self) -> usize {
        match self {
            MapType::Beginner => 12,
            MapType::Master => 18,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectorType {
    Comet,       // 彗星
    Asteroid,    // 小行星
    DwarfPlanet, // 矮行星
    Nebula,      // 气体云
    X,
    Space, // 空域
}

impl std::fmt::Display for SectorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SectorType::Comet => "彗星",
            SectorType::Asteroid => "小行星",
            SectorType::DwarfPlanet => "矮行星",
            SectorType::Nebula => "气体云",
            SectorType::X => "X",
            SectorType::Space => "空域",
        };
        write!(f, "{}", s)
    }
}

impl Map {
    pub fn new(rng: SmallRng, r#type: MapType) -> anyhow::Result<Self> {
        let sectors = MapGenerator::new(rng, &r#type).generate_sectors()?;
        Ok(Self {
            r#type,
            sectors: Sectors { data: sectors },
        })
    }
}

impl rand::distr::Distribution<SectorType> for StandardUniform {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> SectorType {
        match rng.random_range(0..=5) {
            0 => SectorType::Comet,
            1 => SectorType::Asteroid,
            2 => SectorType::DwarfPlanet,
            3 => SectorType::Nebula,
            4 => SectorType::X,
            _ => SectorType::Space,
        }
    }
}
