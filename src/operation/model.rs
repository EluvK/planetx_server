use serde::{Deserialize, Serialize};

use crate::map::{Clue, SectorType};

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

// result

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationResult {
    Survey(usize),
    Target(SectorType),
    Research(Clue), // ABCDEFX1X2
    Locate(bool),
    ReadyPublish(usize),
    DoPublish((usize, SectorType)),
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
        assert!(json_str.contains(r#""survey":{"sector_type":"space","start":1,"end":2}"#));
    }

    #[test]
    fn test_operation_result_json() {
        let result = OperationResult::DoPublish((1, SectorType::Asteroid));
        let res_str = serde_json::to_string(&result).unwrap();
        println!("{}", res_str);
        assert_eq!(res_str, r#"{"do_publish":[1,"asteroid"]}"#);

        let research = OperationResult::Research(Clue {
            subject: SectorType::Asteroid,
            object: SectorType::DwarfPlanet,
            conn: crate::map::ClueConnection::NotAdjacent,
        });
        let res_str = serde_json::to_string(&research).unwrap();
        println!("{}", res_str);
        assert_eq!(res_str, r#"{"research":"没有 小行星 和 矮行星 相邻"}"#);

        let locate = OperationResult::Locate(true);
        let res_str = serde_json::to_string(&locate).unwrap();
        println!("{}", res_str);
        assert_eq!(res_str, r#"{"locate":true}"#);
    }
}
