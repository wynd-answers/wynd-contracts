use crate::ContractError;

pub type R3 = String;

pub fn validate_r3(input: String) -> Result<R3, ContractError> {
    let lower = input.to_lowercase();
    if lower.len() != 15 || !is_hex(&lower) {
        Err(ContractError::InvalidR3(input))
    } else {
        Ok(lower)
    }
}

fn is_hex(input: &str) -> bool {
    input.chars().all(|b| matches!(b, '0'..='9' | 'a'..='f'))
}
