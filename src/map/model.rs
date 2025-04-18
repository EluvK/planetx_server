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
        false
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
                        let distance = (a.index as isize - b.index as isize).unsigned_abs();
                        let wrapped_distance = self.data.len() - distance;
                        distance.min(wrapped_distance) + 1 // 计算最小距离
                    })
            })
            .max() // 找到最大距离
            .unwrap_or(0) // 如果没有匹配项，返回 0
    }

    // survey the sectors in range [st, ed], and count the number of sectors with type object.
    pub fn get_range_type_cnt(&self, st: usize, ed: usize, object: &SectorType) -> usize {
        self.data
            .iter()
            .filter(|s| {
                in_range(st, ed, s.index, self.data.len())
                    && match object {
                        SectorType::Space => {
                            s.r#type == SectorType::Space || s.r#type == SectorType::X
                        }
                        _ => s.r#type == *object,
                    }
            })
            .count()
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

    pub fn meeting_points(&self) -> Vec<(usize, usize)> {
        match self {
            MapType::Standard => [3, 6, 9, 12].iter().map(|&x| (x, 5)).collect(),
            MapType::Expert => [3, 6, 9, 12, 15, 18].iter().map(|&x| (x, 5)).collect(),
        }
    }

    pub fn xclue_points(&self) -> Vec<(usize, usize)> {
        match self {
            MapType::Standard => vec![(10, 5)],
            MapType::Expert => vec![(7, 5), (16, 5)],
        }
    }

    pub fn generate_tokens(&self, user_id: String, user_index: usize) -> Vec<Token> {
        let mut tokens = vec![];
        for _ in 1..=2 {
            tokens.push(Token::new(SectorType::Comet, &user_id, user_index));
        }
        for _ in 1..=4 {
            tokens.push(Token::new(SectorType::Asteroid, &user_id, user_index));
        }
        for _ in 1..=(match self {
            MapType::Standard => 1,
            MapType::Expert => 4,
        }) {
            tokens.push(Token::new(SectorType::DwarfPlanet, &user_id, user_index));
        }
        for _ in 1..=2 {
            tokens.push(Token::new(SectorType::Nebula, &user_id, user_index));
        }
        tokens
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SecretToken {
    pub user_id: String,
    pub user_index: usize,          // game sequence 1, 2, 3, 4
    pub sector_index: usize,        // 0 for init, 1-12/1-18 is set.
    pub meeting_index: usize,       // 0 for known, 1,2, 3 is just published, // 4 for wrong guess
    pub r#type: Option<SectorType>, // 0/-1 is Some, 123 is None
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Token {
    pub placed: bool,
    pub secret: SecretToken,
    pub r#type: SectorType,
}

impl Token {
    pub fn new(r#type: SectorType, user_id: &str, user_index: usize) -> Self {
        Self {
            placed: false,
            secret: SecretToken {
                user_id: user_id.to_owned(),
                user_index,
                sector_index: 0,  // not used yet
                meeting_index: 0, // not used yet
                r#type: None,     // not used yet
            },
            r#type,
        }
    }

    pub fn is_success_located(&self, r#type: SectorType) -> bool {
        self.r#type == r#type && self.is_success_located_any()
    }
    pub fn is_success_located_any(&self) -> bool {
        self.placed && self.secret.meeting_index != 4 && self.secret.r#type.is_some()
    }

    pub fn is_not_used(&self, r#type: &SectorType) -> bool {
        !self.placed && self.r#type == *r#type
    }

    pub fn is_ready_published(&self, r#type: &SectorType) -> bool {
        self.placed && self.r#type == *r#type && self.secret.sector_index == 0
    }

    pub fn is_revealed_checked(&self) -> bool {
        self.placed && self.secret.r#type.is_some() && self.secret.meeting_index == 0
    }

    pub fn any_ready_published(&self) -> bool {
        self.placed && self.secret.sector_index == 0
    }

    pub fn set_to_be_placed(&mut self) -> &mut Self {
        self.placed = true;
        self
    }

    pub fn any_ready_checked(&self) -> bool {
        self.placed && self.secret.meeting_index == 0 && self.secret.r#type.is_none()
    }

    pub fn set_published(&mut self, sector_index: usize) {
        assert!(self.placed && self.secret.sector_index == 0);
        self.secret.sector_index = sector_index;
        self.secret.meeting_index = 3;
    }

    pub fn push_at_meeting(&mut self, revealed_sectors: &[usize]) {
        if self.placed
            && self.secret.sector_index != 0
            && self.secret.meeting_index > 0
            && self.secret.meeting_index <= 3
            && self.secret.r#type.is_none()
            && !revealed_sectors.contains(&self.secret.sector_index)
        {
            self.secret.meeting_index -= 1;
            // if self.secret.meeting_index == 0 {
            //     self.secret.r#type = Some(self.r#type.clone());
            // }
        }
    }

    pub fn reveal_in_the_end(&mut self) -> bool {
        if self.placed && self.secret.r#type.is_none() {
            self.secret.r#type = Some(self.r#type.clone());
            return true;
        }
        false
    }
}

impl Map {
    pub fn place_holder() -> Self {
        Self {
            r#type: MapType::Standard,
            sectors: Sectors { data: vec![] },
        }
    }
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
        self.sectors.get_range_type_cnt(st, ed, object)
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

    pub fn meeting_check(&self, index: usize, target_type: &SectorType) -> bool {
        let sector = &self.sectors.data[index - 1];
        sector.r#type == *target_type
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
        && input_ed
            .is_none_or(|ed| in_range(start, end, ed, max) && in_range(input_st, end, ed, max))
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
        assert!(validate_index_in_range(1, 9, 9, None, 18));
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
