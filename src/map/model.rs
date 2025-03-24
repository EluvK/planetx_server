use rand::{distr::StandardUniform, rngs::SmallRng};
use serde::{Deserialize, Serialize};

use super::generator::MapGenerator;

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapType {
    Standard, // 12 secotrs.
    Expert,   // 18 sectors.
}

impl MapType {
    pub const fn sector_count(&self) -> usize {
        match self {
            MapType::Standard => 12,
            MapType::Expert => 18,
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

    pub fn size(&self) -> usize {
        self.sectors.data.len()
    }

    pub fn survey_sector(&self, st: usize, ed: usize, object: &SectorType) -> usize {
        self.sectors
            .data
            .iter()
            .filter(|s| {
                in_range(st, ed, s.index, self.size())
                    && match object {
                        SectorType::Space => {
                            s.r#type == SectorType::Space || s.r#type == SectorType::X
                        }
                        _ => s.r#type == *object,
                    }
            })
            .count()
    }

    pub fn target_sector(&self, index: usize) -> SectorType {
        match &self.sectors.data[index - 1].r#type {
            SectorType::X => SectorType::Space,
            rest => rest.clone(),
        }
    }

    pub fn locate_x(
        &self,
        index: usize,
        pre_sector_type: &SectorType,
        next_sector_type: &SectorType,
    ) -> bool {
        let sector = &self.sectors.data[index - 1];
        let next_sector = self.sectors.next(index);
        let pre_sector = self.sectors.prev(index);
        sector.r#type == SectorType::X
            && pre_sector.r#type == *pre_sector_type
            && next_sector.r#type == *next_sector_type
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

pub fn validate_index_in_range(
    start: usize,
    end: usize,
    input_st: usize,
    input_ed: Option<usize>,
    max: usize,
) -> bool {
    assert!(0 < start && start <= max);
    assert!(0 < end && end <= max);

    // is a circle from 1 to max, the input should be in the range of start to end.
    // the input_end can be None, which means the input is a single point.
    // or the input_end can be Some, which means the input is a range, so the input_st should be earlier than input_ed.
    in_range(start, end, input_st, max)
        && input_ed.map_or(true, |ed| {
            in_range(start, end, ed, max) && in_range(input_st, end, ed, max)
        })
}

pub fn in_range(start: usize, end: usize, input: usize, max: usize) -> bool {
    assert!(0 < start && start <= max);
    assert!(0 < end && end <= max);

    if start < end {
        start <= input && input <= end
    } else {
        (start <= input && input <= max) || (1 <= input && input <= end)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_validate_index() {
        assert!(validate_index_in_range(1, 9, 3, None, 18));
        assert!(validate_index_in_range(1, 9, 3, Some(4), 18));
        assert!(!validate_index_in_range(1, 9, 3, Some(2), 18));
        assert!(!validate_index_in_range(1, 9, 10, None, 18));
        assert!(!validate_index_in_range(1, 9, 10, Some(11), 18));
        assert!(!validate_index_in_range(11, 1, 10, None, 18));
        assert!(validate_index_in_range(11, 1, 13, None, 18));
        assert!(validate_index_in_range(11, 1, 13, Some(14), 18));
        assert!(!validate_index_in_range(11, 1, 13, Some(12), 18));
    }
}
