use fake::faker::address::en::*;
use fake::faker::company::en::*;
use fake::faker::internet::en::*;
use fake::faker::lorem::en::*;
use fake::faker::name::en::*;
use fake::faker::phone_number::en::*;
use fake::Fake;
use rand::Rng;
use serde_json::Value;

/// Generate fake data from an OpenAPI schema
/// This is similar to Podam in Java - it generates realistic random data based on schema types
pub fn generate_fake_from_schema(schema: &Value) -> Value {
    // If there's an explicit example, use it
    if let Some(example) = schema.get("example") {
        return example.clone();
    }

    let schema_type = schema.get("type").and_then(|t| t.as_str()).unwrap_or("object");

    match schema_type {
        "object" => generate_fake_object(schema),
        "array" => generate_fake_array(schema),
        "string" => generate_fake_string(schema),
        "integer" => generate_fake_integer(schema),
        "number" => generate_fake_number(schema),
        "boolean" => generate_fake_boolean(),
        _ => Value::Null,
    }
}

fn generate_fake_object(schema: &Value) -> Value {
    let mut obj = serde_json::Map::new();

    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (key, prop_schema) in properties {
            let value = generate_fake_from_schema(prop_schema);
            obj.insert(key.clone(), value);
        }
    }

    // Handle additionalProperties for dynamic objects
    if obj.is_empty() {
        if let Some(additional) = schema.get("additionalProperties") {
            if additional.is_object() {
                // Generate a few sample properties
                for i in 1..=3 {
                    let key = format!("property{}", i);
                    obj.insert(key, generate_fake_from_schema(additional));
                }
            }
        }
    }

    Value::Object(obj)
}

fn generate_fake_array(schema: &Value) -> Value {
    let mut rng = rand::rng();
    
    // Determine array length from schema constraints or default to 1-3 items
    let min_items = schema.get("minItems").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let max_items = schema.get("maxItems").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let count = rng.random_range(min_items..=max_items);

    if let Some(items) = schema.get("items") {
        let arr: Vec<Value> = (0..count)
            .map(|_| generate_fake_from_schema(items))
            .collect();
        Value::Array(arr)
    } else {
        Value::Array(vec![])
    }
}

fn generate_fake_string(schema: &Value) -> Value {
    let mut rng = rand::rng();

    // Check for enum values first
    if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
        if !enum_values.is_empty() {
            let idx = rng.random_range(0..enum_values.len());
            return enum_values[idx].clone();
        }
    }

    // Check for format-specific generation
    if let Some(format) = schema.get("format").and_then(|f| f.as_str()) {
        return match format {
            "date" => Value::String(generate_date()),
            "date-time" => Value::String(generate_datetime()),
            "email" => Value::String(FreeEmail().fake()),
            "uri" | "url" => Value::String(format!("https://{}", DomainSuffix().fake::<String>())),
            "uuid" => Value::String(uuid::Uuid::new_v4().to_string()),
            "hostname" => Value::String(DomainSuffix().fake()),
            "ipv4" => Value::String(IPv4().fake()),
            "ipv6" => Value::String(IPv6().fake()),
            "phone" => Value::String(PhoneNumber().fake()),
            "password" => Value::String(Password(8..16).fake()),
            "byte" => Value::String(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                b"sample data",
            )),
            "binary" => Value::String("binary data placeholder".to_string()),
            _ => Value::String(generate_smart_string(schema)),
        };
    }

    // Use property name hints for smarter generation
    Value::String(generate_smart_string(schema))
}

/// Generate a smart string based on property name patterns
fn generate_smart_string(schema: &Value) -> String {
    let mut rng = rand::rng();
    
    // Get min/max length constraints
    let min_len = schema.get("minLength").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let max_len = schema.get("maxLength").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

    // Check for pattern (regex) - simplified handling
    if let Some(_pattern) = schema.get("pattern").and_then(|p| p.as_str()) {
        // For patterns, just generate a placeholder that indicates the pattern
        return "pattern-matched-string".to_string();
    }

    // Default generation - use lorem ipsum words
    let word_count = rng.random_range(1..=3);
    let words: Vec<String> = (0..word_count).map(|_| Word().fake()).collect();
    let result = words.join(" ");

    // Respect length constraints
    if result.len() < min_len {
        let padding: String = Words(1..(min_len / 4 + 2)).fake::<Vec<String>>().join(" ");
        return format!("{} {}", result, padding)[..max_len.min(result.len() + padding.len() + 1)].to_string();
    }
    if result.len() > max_len && max_len > 0 {
        return result[..max_len].to_string();
    }

    result
}

fn generate_fake_integer(schema: &Value) -> Value {
    let mut rng = rand::rng();

    // Check for enum values first
    if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
        if !enum_values.is_empty() {
            let idx = rng.random_range(0..enum_values.len());
            return enum_values[idx].clone();
        }
    }

    // Handle format
    let format = schema.get("format").and_then(|f| f.as_str()).unwrap_or("");
    
    // Get constraints
    let minimum = schema.get("minimum").and_then(|v| v.as_i64()).unwrap_or(0);
    let maximum = schema.get("maximum").and_then(|v| v.as_i64()).unwrap_or(match format {
        "int32" => i32::MAX as i64,
        "int64" => 999999,
        _ => 9999,
    });
    let exclusive_min = schema.get("exclusiveMinimum").and_then(|v| v.as_bool()).unwrap_or(false);
    let exclusive_max = schema.get("exclusiveMaximum").and_then(|v| v.as_bool()).unwrap_or(false);

    let min = if exclusive_min { minimum + 1 } else { minimum };
    let max = if exclusive_max { maximum - 1 } else { maximum };

    let value = rng.random_range(min..=max);
    Value::Number(serde_json::Number::from(value))
}

fn generate_fake_number(schema: &Value) -> Value {
    let mut rng = rand::rng();

    // Check for enum values first
    if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
        if !enum_values.is_empty() {
            let idx = rng.random_range(0..enum_values.len());
            return enum_values[idx].clone();
        }
    }

    // Get constraints
    let minimum = schema.get("minimum").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let maximum = schema.get("maximum").and_then(|v| v.as_f64()).unwrap_or(9999.99);

    let value = rng.random_range(minimum..maximum);
    
    // Round to 2 decimal places for readability
    let rounded = (value * 100.0).round() / 100.0;
    
    if let Some(n) = serde_json::Number::from_f64(rounded) {
        Value::Number(n)
    } else {
        Value::Number(serde_json::Number::from(0))
    }
}

fn generate_fake_boolean() -> Value {
    let mut rng = rand::rng();
    Value::Bool(rng.random_bool(0.5))
}

fn generate_date() -> String {
    let mut rng = rand::rng();
    let year = rng.random_range(2020..=2025);
    let month = rng.random_range(1..=12);
    let day = rng.random_range(1..=28);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn generate_datetime() -> String {
    let mut rng = rand::rng();
    let year = rng.random_range(2020..=2025);
    let month = rng.random_range(1..=12);
    let day = rng.random_range(1..=28);
    let hour = rng.random_range(0..=23);
    let minute = rng.random_range(0..=59);
    let second = rng.random_range(0..=59);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// Generate fake data with property name awareness
/// This is called from the import module to generate smarter mock responses
pub fn generate_fake_from_schema_with_hints(schema: &Value, property_name: Option<&str>) -> Value {
    // If there's an explicit example, use it
    if let Some(example) = schema.get("example") {
        return example.clone();
    }

    let schema_type = schema.get("type").and_then(|t| t.as_str()).unwrap_or("object");

    // For strings, use property name hints for smarter generation
    if schema_type == "string" {
        if let Some(name) = property_name {
            return Value::String(generate_from_property_name(name, schema));
        }
    }

    generate_fake_from_schema(schema)
}

/// Generate a value based on common property name patterns
fn generate_from_property_name(name: &str, schema: &Value) -> String {
    let name_lower = name.to_lowercase();

    // Check format first
    if let Some(format) = schema.get("format").and_then(|f| f.as_str()) {
        if let Value::String(s) = generate_fake_string(schema) {
            return s;
        }
    }

    // Common name patterns
    match name_lower.as_str() {
        // Identity
        "id" | "uuid" | "guid" => uuid::Uuid::new_v4().to_string(),
        
        // Names
        "name" | "fullname" | "full_name" => format!("{} {}", FirstName().fake::<String>(), LastName().fake::<String>()),
        "firstname" | "first_name" | "givenname" => FirstName().fake(),
        "lastname" | "last_name" | "familyname" | "surname" => LastName().fake(),
        "username" | "user_name" | "login" => Username().fake(),
        "nickname" => Username().fake(),
        
        // Contact
        "email" | "emailaddress" | "email_address" => FreeEmail().fake(),
        "phone" | "phonenumber" | "phone_number" | "mobile" | "telephone" => PhoneNumber().fake(),
        
        // Address
        "address" | "streetaddress" | "street_address" => format!("{} {} Street", rand::rng().random_range(100..9999), Word().fake::<String>()),
        "city" | "cityname" => CityName().fake(),
        "state" | "statename" => StateName().fake(),
        "country" | "countryname" => CountryName().fake(),
        "zipcode" | "zip_code" | "postalcode" | "postal_code" | "zip" => PostCode().fake(),
        "latitude" | "lat" => format!("{:.6}", rand::rng().random_range(-90.0..90.0f64)),
        "longitude" | "lng" | "lon" => format!("{:.6}", rand::rng().random_range(-180.0..180.0f64)),
        
        // Internet
        "url" | "website" | "homepage" | "link" => format!("https://www.{}", DomainSuffix().fake::<String>()),
        "domain" | "domainname" | "hostname" => DomainSuffix().fake(),
        "ip" | "ipaddress" | "ip_address" => IPv4().fake(),
        
        // Company
        "company" | "companyname" | "company_name" | "organization" => CompanyName().fake(),
        "jobtitle" | "job_title" | "position" => Profession().fake(),
        "department" => Industry().fake(),
        
        // Text content
        "description" | "desc" | "summary" | "bio" | "about" => Sentences(2..4).fake::<Vec<String>>().join(" "),
        "comment" | "note" | "notes" | "message" | "text" | "content" => Sentence(3..8).fake(),
        "title" | "headline" | "subject" => Words(2..5).fake::<Vec<String>>().join(" "),
        
        // Dates
        "date" | "createdat" | "created_at" | "updatedat" | "updated_at" | 
        "timestamp" | "datetime" | "date_time" => generate_datetime(),
        "birthday" | "birthdate" | "dob" | "dateofbirth" => generate_date(),
        
        // Status/Type
        "status" => {
            let statuses = ["active", "inactive", "pending", "approved", "rejected"];
            statuses[rand::rng().random_range(0..statuses.len())].to_string()
        }
        "type" | "kind" | "category" => {
            let types = ["standard", "premium", "basic", "advanced"];
            types[rand::rng().random_range(0..types.len())].to_string()
        }
        "role" => {
            let roles = ["admin", "user", "moderator", "guest"];
            roles[rand::rng().random_range(0..roles.len())].to_string()
        }
        "gender" | "sex" => {
            let genders = ["male", "female", "other"];
            genders[rand::rng().random_range(0..genders.len())].to_string()
        }
        
        // Currency/Money
        "currency" | "currencycode" => {
            let currencies = ["USD", "EUR", "GBP", "JPY", "CAD"];
            currencies[rand::rng().random_range(0..currencies.len())].to_string()
        }
        "price" | "amount" | "cost" | "total" => format!("{:.2}", rand::rng().random_range(1.0..1000.0f64)),
        
        // Codes/References
        "code" | "sku" | "productcode" | "product_code" => {
            format!("{}{}{}",
                (b'A' + rand::rng().random_range(0..26u8)) as char,
                (b'A' + rand::rng().random_range(0..26u8)) as char,
                rand::rng().random_range(1000..9999)
            )
        }
        
        // Images/Media
        "image" | "imageurl" | "image_url" | "avatar" | "photo" | "picture" => {
            format!("https://picsum.photos/id/{}/200/200", rand::rng().random_range(1..1000))
        }
        
        // Password (masked for security)
        "password" | "pass" | "secret" => Password(12..16).fake(),
        
        // Default - generate lorem words
        _ => {
            let words: Vec<String> = Words(1..3).fake();
            words.join(" ")
        }
    }
}

/// Generate a complete JSON string from a schema
pub fn generate_fake_json(schema: &Value) -> String {
    let fake_value = generate_fake_from_schema(schema);
    serde_json::to_string_pretty(&fake_value).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_object() {
        let schema = json!({
            "type": "object",
            "properties": {
                "id": { "type": "integer" },
                "name": { "type": "string" },
                "email": { "type": "string", "format": "email" }
            }
        });

        let result = generate_fake_from_schema(&schema);
        assert!(result.is_object());
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("email"));
    }

    #[test]
    fn test_array() {
        let schema = json!({
            "type": "array",
            "items": {
                "type": "string"
            }
        });

        let result = generate_fake_from_schema(&schema);
        assert!(result.is_array());
    }

    #[test]
    fn test_enum() {
        let schema = json!({
            "type": "string",
            "enum": ["active", "inactive", "pending"]
        });

        let result = generate_fake_from_schema(&schema);
        assert!(result.is_string());
        let s = result.as_str().unwrap();
        assert!(s == "active" || s == "inactive" || s == "pending");
    }
}
