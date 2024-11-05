use std::collections::BTreeMap;

use mlua::{Lua, Value as LuaValue};
use serde_json::{Number, Value as JsonValue};


#[derive(Debug)]
pub enum ConversionError {
    UnsupportedType(String),
    InvalidTableKey(String),
    InvalidNumber(String),
    LuaError(mlua::Error),
    NonStringKeyInObject,
    MixedArrayKeys,
}

impl ConversionError {
    pub fn unsupported_type(typ: impl ToString) -> Self {
        Self::UnsupportedType(typ.to_string())
    }

    pub fn invalid_table_key(key: impl ToString) -> Self {
        Self::InvalidTableKey(key.to_string())
    }

    pub fn invalid_number(num: impl ToString) -> Self {
        Self::InvalidNumber(num.to_string())
    }

    pub fn lua_error(err: mlua::Error) -> Self {
        Self::LuaError(err)
    }
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedType(t) => write!(f, "Unsupported type: {}", t),
            Self::InvalidTableKey(k) => write!(f, "Invalid table key: {}", k),
            Self::InvalidNumber(n) => write!(f, "Invalid number: {}", n),
            Self::LuaError(e) => write!(f, "Lua error: {}", e),
            Self::NonStringKeyInObject => write!(f, "Non-string key in object"),
            Self::MixedArrayKeys => write!(f, "Mixed array keys"),
        }
    }
}

impl std::error::Error for ConversionError {}

impl From<mlua::Error> for ConversionError {
    fn from(err: mlua::Error) -> Self {
        Self::lua_error(err)
    }
}

// Now we can update the conversion functions to use these constructors:

pub fn lua_value_to_json_value(
    v: LuaValue,
    empty_table_is_array: bool,
) -> Result<JsonValue, ConversionError> {
    match v {
        LuaValue::Nil => Ok(JsonValue::Null),
        LuaValue::Boolean(b) => Ok(JsonValue::Bool(b)),
        LuaValue::Integer(i) => Ok(JsonValue::Number(Number::from(i))),
        LuaValue::Number(n) => {
            if !n.is_finite() {
                return Err(ConversionError::invalid_number(n));
            }
            Ok(JsonValue::Number(
                Number::from_f64(n).ok_or_else(|| ConversionError::invalid_number(n))?,
            ))
        }
        LuaValue::String(s) => Ok(JsonValue::String(s.to_str()?.to_string())),
        LuaValue::Table(table) => lua_table_to_json(table, empty_table_is_array),
        LuaValue::LightUserData(_) => Err(ConversionError::unsupported_type("LightUserData")),
        LuaValue::Function(_) => Err(ConversionError::unsupported_type("Function")),
        LuaValue::Thread(_) => Err(ConversionError::unsupported_type("Thread")),
        LuaValue::UserData(_) => Err(ConversionError::unsupported_type("UserData")),
        LuaValue::Error(_) => Err(ConversionError::unsupported_type("Error")),
        _ => Err(ConversionError::unsupported_type("Unknown")),
    }
}

pub fn json_value_to_lua_value<'a>(
    lua: &'a Lua,
    value: &JsonValue,
) -> Result<LuaValue<'a>, ConversionError> {
    match value {
        JsonValue::Null => Ok(LuaValue::Nil),
        JsonValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i as i32))
            } else if let Some(f) = n.as_f64() {
                if f.is_finite() {
                    Ok(LuaValue::Number(f))
                } else {
                    Err(ConversionError::invalid_number(f))
                }
            } else {
                Err(ConversionError::invalid_number(n))
            }
        }
        JsonValue::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        JsonValue::Array(arr) => {
            let table = lua.create_table_with_capacity(arr.len(), 0)?;
            for (i, value) in arr.iter().enumerate() {
                table.set(i + 1, json_value_to_lua_value(lua, value)?)?;
            }
            Ok(LuaValue::Table(table))
        }
        JsonValue::Object(obj) => {
            let table = lua.create_table_with_capacity(0, obj.len())?;
            for (key, value) in obj {
                table.set(key.clone(), json_value_to_lua_value(lua, value)?)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

fn to_array_index(key: &LuaValue) -> Option<usize> {
    match key {
        LuaValue::Integer(i) if *i > 0 => Some(*i as usize),
        LuaValue::Number(n) if n.is_finite() && n.fract() == 0.0 && *n > 0.0 => Some(*n as usize),
        _ => None,
    }
}

pub fn lua_table_to_json(
    table: mlua::Table,
    empty_table_is_array: bool,
) -> Result<JsonValue, ConversionError> {
    let len = table.len()? as usize;

    eprintln!("lua_table_to_json: table.len={}", len);

    let mut is_array = true;
    let mut is_object = true;

    let mut array = vec![];
    let mut object = BTreeMap::new();

    let mut is_empty = true;
    for pair in table.pairs::<LuaValue, LuaValue>() {
        is_empty = false;
        let (key, value) = pair?;

        if let Some(index) = to_array_index(&key) {
            is_object = false;
            if !is_array {
                break;
            }
            // Ensure the vector is long enough
            let i = index - 1;
            if i + 1 > array.len() {
                let n = i + 1 - array.len();
                for _ in 0..n {
                    array.push(LuaValue::Nil);
                }
            }
            // Now we can update the value
            array[i] = value;
            // TODO: Should we have a maximum gap size?
        } else {
            is_array = false;
            if !is_object {
                break;
            }
            if let LuaValue::String(s) = key {
                let ss = s.to_str()?;
                object.insert(ss.to_string(), value);
            } else {
                is_object = false;
                break;
            }
        }
    }

    if is_empty {
        if empty_table_is_array {
            return Ok(serde_json::json!([]));
        } else {
            return Ok(serde_json::json!({}));
        }
    }

    if !is_array && !is_object {
        //TODO: Add more details
        return Err(ConversionError::invalid_table_key("invalid table key"));
    }

    if is_array {
        let mut json_array = Vec::with_capacity(array.len());
        for value in array {
            json_array.push(lua_value_to_json_value(value, empty_table_is_array)?);
        }
        Ok(JsonValue::Array(json_array))
    } else {
        let mut map = serde_json::Map::with_capacity(object.len());
        for (key, value) in object {
            map.insert(key, lua_value_to_json_value(value, empty_table_is_array)?);
        }
        Ok(JsonValue::Object(map))
    }
}

// Testing the new error constructors
#[cfg(test)]
mod json_tests {
    use super::*;

    use mlua::Lua;
    use serde_json::json;

    #[test]
    fn test_error_constructors() {
        let err = ConversionError::unsupported_type("CustomType");
        assert!(matches!(err, ConversionError::UnsupportedType(_)));

        let err = ConversionError::invalid_number(f64::INFINITY);
        assert!(matches!(err, ConversionError::InvalidNumber(_)));

        let err = ConversionError::invalid_table_key("invalid key");
        assert!(matches!(err, ConversionError::InvalidTableKey(_)));
    }

    #[test]
    fn test_roundtrip_array() -> Result<(), Box<dyn std::error::Error>> {
        let lua = Lua::new();

        // Test various array types
        let test_cases = vec![
            json!([]),
            json!([1, 2, 3]),
            // We can't handle a null as the last entry in an array, but we shoulud be able to handle an internal null,
            json!([1, "two", null, true,]),
            json!([[1, 2], ["a", "b"]]),
        ];

        for case in test_cases {
            let lua_table = json_value_to_lua_value(&lua, &case)?;
            let round_trip = lua_value_to_json_value(lua_table, true)?;
            assert_eq!(case, round_trip, "Failed roundtrip for case: {}", case);
        }

        Ok(())
    }

    #[test]
    fn test_roundtrip_object() -> Result<(), Box<dyn std::error::Error>> {
        let lua = Lua::new();

        // Test various object types
        let test_cases = vec![
            json!({}),
            json!({"a": 1, "b": 2}),
            json!({"nested": {"array": [1,2,3], "obj": {"x": true}}}),
            json!({"mixed": [{"a": 1}, null, [1,2,3]]}),
        ];

        for case in test_cases {
            eprintln!("test_roundtrip_object: case='{}'", case);
            let lua_table = json_value_to_lua_value(&lua, &case)?;
            eprintln!("lua_table: {:?}", lua_table);
            let round_trip = lua_value_to_json_value(lua_table, false)?;
            assert_eq!(case, round_trip, "Failed roundtrip for case: {}", case);
        }

        Ok(())
    }

    #[test]
    fn test_error_cases() -> Result<(), Box<dyn std::error::Error>> {
        let lua = Lua::new();

        // Test function conversion
        let func = lua.create_function(|_, ()| Ok(()))?;
        assert!(matches!(
            lua_value_to_json_value(LuaValue::Function(func), false),
            Err(ConversionError::UnsupportedType(_))
        ));

        // Test invalid numbers
        let infinity = std::f64::INFINITY;
        assert!(matches!(
            lua_value_to_json_value(LuaValue::Number(infinity), false),
            Err(ConversionError::InvalidNumber(_))
        ));

        Ok(())
    }
}
