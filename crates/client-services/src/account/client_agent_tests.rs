//! 宿主自报的 User-Agent 的拼装（#8201 迁移计划 P11）：逐条跑 `shared/contracts/client-agent/cases.json`，拼出的每一条都要是 [`valid_client_agent`] 认的形状。

use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Contract {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    platform: String,
    version: String,
    device: Option<String>,
    system: Option<String>,
    edition: String,
    agent: Option<String>,
    error: Option<String>,
    /// 有意与 Android 原来的写法不同的用例上的说明，测试不读。
    #[serde(rename = "note")]
    _note: Option<String>,
}

fn contract_cases() -> Vec<Case> {
    let contract: Contract = serde_json::from_str(include_str!(
        "../../../../shared/contracts/client-agent/cases.json"
    ))
    .unwrap();
    contract.cases
}

#[test]
fn shared_contract_cases() {
    let cases = contract_cases();
    assert!(cases.len() >= 20);
    for case in &cases {
        let actual = client_agent(
            &case.platform,
            &case.version,
            case.device.as_deref(),
            case.system.as_deref(),
            &case.edition,
        );
        let label = format!("{}/{:?}/{:?}", case.platform, case.version, case.device);
        match (&case.agent, &case.error) {
            (Some(agent), None) => {
                let actual = actual.unwrap_or_else(|error| panic!("{label}: {error:?}"));
                assert_eq!(&actual, agent, "{label}");
                assert!(valid_client_agent(&actual), "{label}: {actual}");
            }
            (None, Some(error)) => {
                assert_eq!(actual.unwrap_err().code(), error, "{label}");
            }
            _ => panic!("{label}: a case needs exactly one of agent and error"),
        }
    }
}

#[test]
fn every_part_at_its_longest_still_fits_the_backend_shape() {
    let long = "x".repeat(200);
    let agent = client_agent("harmony", &"9".repeat(40), Some(&long), Some(&long), &long).unwrap();
    assert!(valid_client_agent(&agent), "{agent}");
    assert!(agent.len() <= 256, "{}", agent.len());
    assert!(BackendAccountClient::with_user_agent(&agent).is_ok());
}
