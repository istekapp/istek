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
            "email" => Value::String(generate_email()),
            "uri" | "url" => Value::String(generate_url()),
            "uuid" => Value::String(uuid::Uuid::new_v4().to_string()),
            "hostname" => Value::String(generate_domain()),
            "ipv4" => Value::String(generate_ipv4()),
            "ipv6" => Value::String(generate_ipv6()),
            "phone" => Value::String(generate_phone()),
            "password" => Value::String(generate_password()),
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
    if schema.get("pattern").and_then(|p| p.as_str()).is_some() {
        return "pattern-matched-string".to_string();
    }

    // Default generation - use random words
    let word_count = rng.random_range(1..=3);
    let words: Vec<String> = (0..word_count).map(|_| generate_word()).collect();
    let result = words.join(" ");

    // Respect length constraints
    if result.len() < min_len {
        let padding = generate_words(min_len / 4 + 2);
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

// ============================================================================
// Simple cross-platform fake data generators (no external faker dependencies)
// ============================================================================

const FIRST_NAMES: &[&str] = &[
    "James", "Mary", "John", "Patricia", "Robert", "Jennifer", "Michael", "Linda",
    "William", "Elizabeth", "David", "Barbara", "Richard", "Susan", "Joseph", "Jessica",
    "Thomas", "Sarah", "Charles", "Karen", "Christopher", "Lisa", "Daniel", "Nancy"
];

const LAST_NAMES: &[&str] = &[
    "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis",
    "Rodriguez", "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson",
    "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee", "Perez", "Thompson", "White"
];

const CITIES: &[&str] = &[
    "New York", "Los Angeles", "Chicago", "Houston", "Phoenix", "Philadelphia",
    "San Antonio", "San Diego", "Dallas", "San Jose", "Austin", "Jacksonville",
    "Fort Worth", "Columbus", "Charlotte", "Seattle", "Denver", "Boston", "Portland"
];

const STATES: &[&str] = &[
    "California", "Texas", "Florida", "New York", "Pennsylvania", "Illinois",
    "Ohio", "Georgia", "North Carolina", "Michigan", "New Jersey", "Virginia",
    "Washington", "Arizona", "Massachusetts", "Tennessee", "Indiana", "Missouri"
];

const COUNTRIES: &[&str] = &[
    "United States", "Canada", "United Kingdom", "Germany", "France", "Australia",
    "Japan", "Brazil", "India", "Mexico", "Spain", "Italy", "Netherlands", "Sweden"
];

const COMPANIES: &[&str] = &[
    "Acme Corp", "Globex", "Initech", "Umbrella Corp", "Stark Industries",
    "Wayne Enterprises", "Cyberdyne Systems", "Tyrell Corporation", "Massive Dynamic",
    "Soylent Corp", "Oscorp", "LexCorp", "Aperture Science", "Black Mesa"
];

const JOB_TITLES: &[&str] = &[
    "Software Engineer", "Product Manager", "Data Scientist", "UX Designer",
    "DevOps Engineer", "Marketing Manager", "Sales Representative", "HR Specialist",
    "Financial Analyst", "Project Manager", "QA Engineer", "Technical Writer"
];

const DEPARTMENTS: &[&str] = &[
    "Engineering", "Marketing", "Sales", "Human Resources", "Finance",
    "Operations", "Customer Support", "Research", "Legal", "IT"
];

const WORDS: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit",
    "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore",
    "magna", "aliqua", "enim", "ad", "minim", "veniam", "quis", "nostrud"
];

const DOMAINS: &[&str] = &[
    "example.com", "test.com", "demo.org", "sample.net", "mock.io", "fake.dev"
];

fn generate_first_name() -> String {
    let mut rng = rand::rng();
    FIRST_NAMES[rng.random_range(0..FIRST_NAMES.len())].to_string()
}

fn generate_last_name() -> String {
    let mut rng = rand::rng();
    LAST_NAMES[rng.random_range(0..LAST_NAMES.len())].to_string()
}

fn generate_full_name() -> String {
    format!("{} {}", generate_first_name(), generate_last_name())
}

fn generate_username() -> String {
    let mut rng = rand::rng();
    let first = generate_first_name().to_lowercase();
    let num: u32 = rng.random_range(1..999);
    format!("{}{}", first, num)
}

fn generate_email() -> String {
    let mut rng = rand::rng();
    let username = generate_username();
    let domain = DOMAINS[rng.random_range(0..DOMAINS.len())];
    format!("{}@{}", username, domain)
}

fn generate_phone() -> String {
    let mut rng = rand::rng();
    format!(
        "+1-{:03}-{:03}-{:04}",
        rng.random_range(200..999),
        rng.random_range(200..999),
        rng.random_range(1000..9999)
    )
}

fn generate_city() -> String {
    let mut rng = rand::rng();
    CITIES[rng.random_range(0..CITIES.len())].to_string()
}

fn generate_state() -> String {
    let mut rng = rand::rng();
    STATES[rng.random_range(0..STATES.len())].to_string()
}

fn generate_country() -> String {
    let mut rng = rand::rng();
    COUNTRIES[rng.random_range(0..COUNTRIES.len())].to_string()
}

fn generate_zipcode() -> String {
    let mut rng = rand::rng();
    format!("{:05}", rng.random_range(10000..99999))
}

fn generate_street_address() -> String {
    let mut rng = rand::rng();
    let num = rng.random_range(100..9999);
    let streets = ["Main St", "Oak Ave", "Maple Dr", "Park Blvd", "Cedar Ln", "Pine Rd"];
    let street = streets[rng.random_range(0..streets.len())];
    format!("{} {}", num, street)
}

fn generate_url() -> String {
    let mut rng = rand::rng();
    let domain = DOMAINS[rng.random_range(0..DOMAINS.len())];
    format!("https://www.{}", domain)
}

fn generate_domain() -> String {
    let mut rng = rand::rng();
    DOMAINS[rng.random_range(0..DOMAINS.len())].to_string()
}

fn generate_ipv4() -> String {
    let mut rng = rand::rng();
    format!(
        "{}.{}.{}.{}",
        rng.random_range(1..255),
        rng.random_range(0..255),
        rng.random_range(0..255),
        rng.random_range(1..255)
    )
}

fn generate_ipv6() -> String {
    let mut rng = rand::rng();
    let parts: Vec<String> = (0..8)
        .map(|_| format!("{:04x}", rng.random_range(0..65535u32)))
        .collect();
    parts.join(":")
}

fn generate_company() -> String {
    let mut rng = rand::rng();
    COMPANIES[rng.random_range(0..COMPANIES.len())].to_string()
}

fn generate_job_title() -> String {
    let mut rng = rand::rng();
    JOB_TITLES[rng.random_range(0..JOB_TITLES.len())].to_string()
}

fn generate_department() -> String {
    let mut rng = rand::rng();
    DEPARTMENTS[rng.random_range(0..DEPARTMENTS.len())].to_string()
}

fn generate_word() -> String {
    let mut rng = rand::rng();
    WORDS[rng.random_range(0..WORDS.len())].to_string()
}

fn generate_words(count: usize) -> String {
    (0..count).map(|_| generate_word()).collect::<Vec<_>>().join(" ")
}

fn generate_sentence(word_count: usize) -> String {
    let words = generate_words(word_count);
    let mut chars: Vec<char> = words.chars().collect();
    if !chars.is_empty() {
        chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
    }
    format!("{}.", chars.into_iter().collect::<String>())
}

fn generate_sentences(count: usize) -> String {
    let mut rng = rand::rng();
    (0..count)
        .map(|_| generate_sentence(rng.random_range(5..12)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_password() -> String {
    let mut rng = rand::rng();
    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%";
    let chars: Vec<char> = chars.chars().collect();
    (0..12)
        .map(|_| chars[rng.random_range(0..chars.len())])
        .collect()
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
    if schema.get("format").and_then(|f| f.as_str()).is_some() {
        if let Value::String(s) = generate_fake_string(schema) {
            return s;
        }
    }

    // Common name patterns
    match name_lower.as_str() {
        // Identity
        "id" | "uuid" | "guid" => uuid::Uuid::new_v4().to_string(),
        
        // Names
        "name" | "fullname" | "full_name" => generate_full_name(),
        "firstname" | "first_name" | "givenname" => generate_first_name(),
        "lastname" | "last_name" | "familyname" | "surname" => generate_last_name(),
        "username" | "user_name" | "login" => generate_username(),
        "nickname" => generate_username(),
        
        // Contact
        "email" | "emailaddress" | "email_address" => generate_email(),
        "phone" | "phonenumber" | "phone_number" | "mobile" | "telephone" => generate_phone(),
        
        // Address
        "address" | "streetaddress" | "street_address" => generate_street_address(),
        "city" | "cityname" => generate_city(),
        "state" | "statename" => generate_state(),
        "country" | "countryname" => generate_country(),
        "zipcode" | "zip_code" | "postalcode" | "postal_code" | "zip" => generate_zipcode(),
        "latitude" | "lat" => format!("{:.6}", rand::rng().random_range(-90.0..90.0f64)),
        "longitude" | "lng" | "lon" => format!("{:.6}", rand::rng().random_range(-180.0..180.0f64)),
        
        // Internet
        "url" | "website" | "homepage" | "link" => generate_url(),
        "domain" | "domainname" | "hostname" => generate_domain(),
        "ip" | "ipaddress" | "ip_address" => generate_ipv4(),
        
        // Company
        "company" | "companyname" | "company_name" | "organization" => generate_company(),
        "jobtitle" | "job_title" | "position" => generate_job_title(),
        "department" => generate_department(),
        
        // Text content
        "description" | "desc" | "summary" | "bio" | "about" => generate_sentences(3),
        "comment" | "note" | "notes" | "message" | "text" | "content" => generate_sentence(8),
        "title" | "headline" | "subject" => generate_words(4),
        
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
            let mut rng = rand::rng();
            format!("{}{}{}",
                (b'A' + rng.random_range(0..26u8)) as char,
                (b'A' + rng.random_range(0..26u8)) as char,
                rng.random_range(1000..9999)
            )
        }
        
        // Images/Media
        "image" | "imageurl" | "image_url" | "avatar" | "photo" | "picture" => {
            format!("https://picsum.photos/id/{}/200/200", rand::rng().random_range(1..1000))
        }
        
        // Password (masked for security)
        "password" | "pass" | "secret" => generate_password(),
        
        // Default - generate lorem words
        _ => generate_words(2)
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
    use serde_json::json;

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
