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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn is_hex_works() {
        assert!(is_hex(""));
        assert!(is_hex("1234567890abcdef"));
        assert!(!is_hex("1234567890abcdefg"));
        assert!(!is_hex("123A"));
    }

    #[test]
    fn validate_r3_works() {
        // Too long
        validate_r3("1234567890abcdef".into()).unwrap_err();
        // too short
        validate_r3("8362718fffffff".into()).unwrap_err();
        // real one
        validate_r3("8362718ffffffff".into()).unwrap();
        // allow uppercase, but convert it to lowercase
        assert_eq!(
            validate_r3("8362718FFFFFFFF".into()).unwrap().as_str(),
            "8362718ffffffff"
        );
    }
}
