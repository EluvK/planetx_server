use serde::{Deserialize, Serialize};

use crate::map::SectorType;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Survey(SurveyOperatoin),
    Target(TargetOperation),
    Research(ResearchOperation),
    Locate(LocateOperation),
    ReadyPublish(ReadyPublishOperation),
    DoPublish(DoPublishOperation),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SurveyOperatoin {
    pub sector_type: SectorType,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetOperation {
    pub index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchOperation {
    pub index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocateOperation {
    pub index: usize,
    pub pre_sector_type: SectorType,
    pub next_sector_type: SectorType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadyPublishOperation {
    pub sectors: Vec<SectorType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoPublishOperation {
    pub index: usize,
    pub sector_type: SectorType,
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_operation_json() {
        let survey = Operation::Survey(SurveyOperatoin {
            sector_type: SectorType::Space,
            start: 1,
            end: 2,
        });
        let json_str = serde_json::to_string(&survey).unwrap();
        println!("{}", json_str);
    }
}
