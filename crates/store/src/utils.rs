use crate::error::Error;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

pub fn serialize<T>(value: &T) -> Result<Bytes, Error>
where
    T: Serialize,
{
    let json = serde_json::to_vec(value)?;
    Ok(Bytes::from(json))
}

pub fn deserialize<T>(bytes: Bytes) -> Result<T, Error>
where
    T: for<'de> Deserialize<'de>,
{
    let value = serde_json::from_slice(&bytes)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestStruct {
        field: String,
    }

    #[test]
    fn test_serialization() -> Result<(), Error> {
        let test_struct = TestStruct {
            field: "test".to_string(),
        };

        let bytes = serialize(&test_struct)?;
        let deserialized: TestStruct = deserialize(bytes)?;

        assert_eq!(test_struct, deserialized);
        Ok(())
    }
}
