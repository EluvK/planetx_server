use super::generator::MapGenerator;

pub struct Map {
    pub r#type: MapType,
    pub sectors: Vec<Sector>,
}

#[derive(Clone, Debug)]
pub struct Sector {
    pub index: u32, // 1-based index.
    pub r#type: SectorType,
}

#[derive(Clone)]
pub enum MapType {
    Beginner, // 12 secotrs.
    Master,   // 18 sectors.
}

impl MapType {
    pub const fn sector_count(&self) -> u32 {
        match self {
            MapType::Beginner => 12,
            MapType::Master => 18,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SectorType {
    Comet,       // 彗星
    Asteroid,    // 小行星
    DwarfPlanet, // 矮行星
    Nebula,      // 气体云
    X,
    Space, // 空域
}

impl Map {
    pub fn new(r#type: MapType, seed: u64) -> anyhow::Result<Self> {
        let sectors = MapGenerator::new(seed, &r#type).generate_sectors()?;
        Ok(Self { r#type, sectors })
    }
}
