// src/models.rs
use chrono::{DateTime, Utc};
use serde::{
    self, Deserialize, Deserializer, Serialize,
    de::{self, Unexpected, Visitor},
};
use sqlx::Type;
use std::str::FromStr;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};
use uuid::Uuid;
use validator::Validate;

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString, Display, EnumIter, AsRefStr,
)]
#[sqlx(type_name = "product_condition")]
#[strum(ascii_case_insensitive)]
pub enum ProductCondition {
    #[strum(serialize = "Nowy")]
    New,
    #[strum(serialize = "Jak nowy")]
    LikeNew,
    #[strum(serialize = "Bardzo dobry")]
    VeryGood,
    #[strum(serialize = "Dobry")]
    Good,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString, Display, EnumIter, AsRefStr,
)]
#[sqlx(type_name = "product_status")]
#[strum(ascii_case_insensitive)]
pub enum ProductStatus {
    #[strum(serialize = "Dostępny")]
    Available,
    #[strum(serialize = "Zarezerwowany")]
    Reserved,
    #[strum(serialize = "Sprzedany")]
    Sold,
    #[strum(serialize = "Zarchiwizowany")]
    Archived,
}

#[allow(dead_code)]
impl ProductStatus {
    pub fn from_query_param(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            // Używamy to_lowercase() dla case-insensitivity
            "available" => Ok(ProductStatus::Available),
            "reserved" => Ok(ProductStatus::Reserved),
            "sold" => Ok(ProductStatus::Sold),
            "archived" => Ok(ProductStatus::Archived),
            "dostępny" => Ok(ProductStatus::Available),
            "zarezerwowany" => Ok(ProductStatus::Reserved),
            "sprzedany" => Ok(ProductStatus::Sold),
            "zarchiwizowany" => Ok(ProductStatus::Archived),
            _ => Err(format!("Nierozpoznany wariant ProductStatus: '{}'", s)),
        }
    }

    pub fn to_form_value(&self) -> &'static str {
        // Zwraca angielską nazwę
        match self {
            ProductStatus::Available => "Available",
            ProductStatus::Reserved => "Reserved",
            ProductStatus::Sold => "Sold",
            ProductStatus::Archived => "Archived",
        }
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString, Display, EnumIter, AsRefStr,
)]
#[sqlx(type_name = "product_gender")]
#[strum(ascii_case_insensitive)]
pub enum ProductGender {
    Damskie,
    Meskie,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString, Display, EnumIter, AsRefStr,
)]
#[sqlx(type_name = "category_type")]
#[strum(ascii_case_insensitive, serialize_all = "kebab-case")]
pub enum Category {
    #[strum(to_string = "Koszule")] // 'to_string' to alias dla 'serialize'
    Koszule,
    #[strum(to_string = "Spodnie")]
    Spodnie,
    #[strum(to_string = "Sukienki")]
    Sukienki,
    #[strum(to_string = "Spódnice")]
    Spodnice,
    #[strum(to_string = "Swetry")]
    Swetry,
    #[strum(to_string = "Bluzy")]
    Bluzy,
    #[strum(to_string = "Kurtki i Płaszcze")]
    KurtkiPlaszcze,
    #[strum(to_string = "Marynarki i Żakiety")]
    MarynarkiZakiety,
    #[strum(to_string = "Obuwie")]
    Obuwie,
    #[strum(to_string = "Torebki")]
    Torebki,
    #[strum(to_string = "Akcesoria")]
    Akcesoria,
    #[strum(to_string = "Bielizna")]
    Bielizna,
    #[strum(to_string = "Stroje kąpielowe")]
    StrojeKapielowe,
    #[strum(to_string = "Inne")]
    Inne,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub price: i64,
    pub gender: ProductGender,
    pub condition: ProductCondition,
    pub category: Category,
    pub status: ProductStatus,
    pub images: Vec<String>,
    pub on_sale: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum Role {
    Admin,
    Customer,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    #[sqlx(rename = "email")]
    pub email: String,
    #[sqlx(rename = "password_hash")]
    #[serde(skip_serializing)]
    pub password_hash: String,
    #[sqlx(rename = "role")]
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
    fn from(user: User) -> Self {
        UserPublic {
            id: user.id,
            email: user.email,
            role: user.role,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

/// Status zamówienia
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString, Display, AsRefStr, EnumIter,
)]
#[sqlx(type_name = "order_status_enum")]
#[sqlx(rename_all = "lowercase")]
#[strum(ascii_case_insensitive)]
pub enum OrderStatus {
    #[strum(serialize = "Oczekujące")]
    Pending,
    #[strum(serialize = "W trakcie realizacji")]
    Processing,
    #[strum(serialize = "Wysłane")]
    Shipped,
    #[strum(serialize = "Dostarczone")]
    Delivered,
    #[strum(serialize = "Anulowane")]
    Cancelled,
}

impl OrderStatus {
    pub fn to_form_value(&self) -> &'static str {
        match self {
            OrderStatus::Pending => "Pending",
            OrderStatus::Processing => "Processing",
            OrderStatus::Shipped => "Shipped",
            OrderStatus::Delivered => "Delivered",
            OrderStatus::Cancelled => "Cancelled",
        }
    }
}

/// Reprezentuje pojedyńczą pozycję w zamówieniu
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Validate)]
pub struct OrderItem {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Uuid,
    pub price_at_purchase: i64,
}

/// Reprezentuje zamówienie
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Validate)]
pub struct Order {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub order_date: DateTime<Utc>,
    pub status: OrderStatus,
    pub total_price: i64,

    #[validate(length(min = 1, max = 100))]
    pub shipping_first_name: String,
    #[validate(length(min = 1, max = 100))]
    pub shipping_last_name: String,

    #[validate(length(min = 1, max = 255))]
    pub shipping_address_line1: String,
    #[validate(length(max = 255))]
    pub shipping_address_line2: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub shipping_city: String,
    #[validate(length(min = 1, max = 20))]
    pub shipping_postal_code: String,
    #[validate(length(min = 1, max = 100))]
    pub shipping_country: String,
    #[validate(length(min = 1, max = 30))]
    pub shipping_phone: String,
    pub payment_method: Option<PaymentMethod>,
    pub shipping_method_name: Option<String>,

    #[validate(email)]
    pub guest_email: Option<String>,
    pub guest_session_id: Option<Uuid>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, sqlx::Type, Display, EnumString)]
#[sqlx(type_name = "payment_method_enum", rename_all = "lowercase")] // Mapowanie na typ SQL i nazwy wariantów w DB
#[strum(ascii_case_insensitive)]
pub enum PaymentMethod {
    #[strum(serialize = "BLIK")]
    Blik,
    #[strum(serialize = "Przelew tradycyjny (P24)")]
    Transfer,
}
// --- STRUKTURY PAYLOAD DLA HANDLERÓW ZAMÓWIEŃ ---

/// Reprezentuje pojedyńczy produkt w payloadzie tworzenia zamówienia
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateOrderFromCartPayload {
    #[validate(length(min = 1, max = 255, message = "Linia adresu wysyłki jest wymagana"))]
    pub shipping_address_line1: String,

    #[validate(length(max = 255))]
    pub shipping_address_line2: Option<String>,

    #[validate(length(min = 1, max = 100, message = "Miasto wysyłki jest wymagane"))]
    pub shipping_city: String,

    #[validate(length(min = 1, max = 20, message = "Kod pocztowy wysyłki jest wymagany"))]
    pub shipping_postal_code: String,

    #[validate(length(min = 1, max = 100, message = "Kraj wysyłki jest wymagany"))]
    pub shipping_country: String,
}

/// Payload do aktualizacji statusu zamówienia
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateOrderStatusPayload {
    pub status: OrderStatus,
}

#[derive(Debug, Serialize)]
pub struct OrderItemDetailsPublic {
    pub order_item_id: Uuid,
    #[serde(flatten)]
    pub product: Product,
    pub price_at_purchase: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderDetailsResponse {
    #[serde(flatten)]
    pub order: Order,
    pub items: Vec<OrderItemDetailsPublic>,
}

// --- STRUKTURY DLA KOSZYKA ZAKUPÓW ---
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ShoppingCart {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub guest_session_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

///Payload dla scalania koszyka
#[derive(Debug, Deserialize)]
pub struct MergeCartPayload {
    pub guest_cart_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct GuestCartOperationResponse {
    pub guest_cart_id: Uuid,
    #[serde(flatten)]
    pub cart_details: CartDetailsResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CartItem {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub product_id: Uuid,
    pub added_at: DateTime<Utc>,
}

// --- STRUKTURY PAYLOAD DLA HANDLERÓW KOSZYKA ---
#[derive(Debug, Clone, Deserialize)]
pub struct AddProductToCartPayload {
    pub product_id: Uuid,
}

// --- STRUKTURY ODPOWIEDZI API DLA KOSZYKA ---
// Do wyświetlania pozycji koszyka wraz z danymi produktu
#[derive(Debug, Serialize, Clone)]
pub struct CartItemPublic {
    pub cart_item_id: Uuid,
    #[serde(flatten)]
    pub product: Product,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Default)]
pub struct CartDetailsResponse {
    pub cart_id: Uuid,
    pub user_id: Option<Uuid>,
    pub items: Vec<CartItemPublic>,
    pub total_items: usize,
    pub total_price: i64,
    pub updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CartItemWithProduct {
    pub cart_item_id: Uuid,      // ci.id AS cart_item_id
    pub added_at: DateTime<Utc>, // ci.added_at
    pub cart_id: Uuid,           // ci.cart_id

    pub product_id: Uuid, // p.id AS product_id (aby odróżnić od ci.product_id jeśli byłby potrzebny)
    pub name: String,     // p.name
    pub description: String, // p.description
    pub price: i64,       // p.price
    pub gender: ProductGender, // p.gender
    pub condition: ProductCondition, // p.condition
    pub category: Category, // p.category
    pub on_sale: bool,
    pub status: ProductStatus, // p.status
    pub images: Vec<String>,   // p.images
}

#[derive(Deserialize, Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserShippingDetails {
    pub user_id: Uuid,
    pub shipping_first_name: Option<String>,
    pub shipping_last_name: Option<String>,
    pub shipping_address_line1: Option<String>,
    pub shipping_address_line2: Option<String>,
    pub shipping_city: Option<String>,
    pub shipping_postal_code: Option<String>,
    pub shipping_country: Option<String>,
    pub shipping_phone: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Struktura dla payloadu z formularza HTMX
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateUserShippingDetailsPayload {
    #[validate(length(max = 100, message = "Imię może mieć maksymalnie 100 znaków."))]
    pub shipping_first_name: Option<String>, // HTML form sends "" for empty, serde makes it Some("")

    #[validate(length(max = 100, message = "Nazwisko może mieć maksymalnie 100 znaków."))]
    pub shipping_last_name: Option<String>,

    #[validate(length(
        max = 255,
        message = "Adres (linia 1) może mieć maksymalnie 255 znaków."
    ))]
    pub shipping_address_line1: Option<String>,

    #[validate(length(
        max = 255,
        message = "Adres (linia 2) może mieć maksymalnie 255 znaków."
    ))]
    pub shipping_address_line2: Option<String>,

    #[validate(length(max = 100, message = "Miasto może mieć maksymalnie 100 znaków."))]
    pub shipping_city: Option<String>,

    #[validate(length(max = 20, message = "Kod pocztowy może mieć maksymalnie 20 znaków."))]
    pub shipping_postal_code: Option<String>,

    #[validate(length(max = 100, message = "Kraj może mieć maksymalnie 100 znaków."))]
    pub shipping_country: Option<String>,

    #[validate(length(max = 30, message = "Numer telefonu może mieć maksymalnie 30 znaków."))]
    // Można dodać walidację regex dla telefonu, np.
    // #[validate(regex(path = "crate::utils::PHONE_REGEX", message = "Nieprawidłowy format numeru telefonu."))]
    pub shipping_phone: Option<String>,
}

impl Default for UserShippingDetails {
    fn default() -> Self {
        Self {
            user_id: Uuid::nil(),
            shipping_first_name: None,
            shipping_last_name: None,
            shipping_address_line1: None,
            shipping_address_line2: None,
            shipping_city: None,
            shipping_postal_code: None,
            shipping_country: None,
            shipping_phone: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CheckoutFormPayload {
    // Dane dostawy
    #[validate(length(min = 1, message = "Imię do wysyłki jest wymagane."))]
    pub shipping_first_name: String,
    #[validate(length(min = 1, message = "Nazwisko do wysyłki jest wymagane."))]
    pub shipping_last_name: String,
    #[validate(length(min = 1, message = "Adres (linia 1) do wysyłki jest wymagany."))]
    pub shipping_address_line1: String,
    pub shipping_address_line2: Option<String>,
    #[validate(length(min = 1, message = "Miasto do wysyłki jest wymagane."))]
    pub shipping_city: String,
    #[validate(length(min = 1, message = "Kod pocztowy do wysyłki jest wymagany."))]
    pub shipping_postal_code: String,
    #[validate(length(min = 1, message = "Kraj do wysyłki jest wymagany."))]
    pub shipping_country: String,
    #[validate(length(min = 1, message = "Telefon do wysyłki jest wymagany."))]
    pub shipping_phone: String,

    // Email dla gościa - staje się wymagany, jeśli użytkownik nie jest zalogowany.
    // Walidację "wymagane jeśli gość" trzeba będzie zrobić w logice handlera.
    #[validate(email(message = "Nieprawidłowy format adresu email."))]
    pub guest_checkout_email: Option<String>, // Pole dla emaila gościa

    pub billing_same_as_shipping: Option<String>,
    pub billing_first_name: Option<String>,
    pub billing_last_name: Option<String>,
    pub billing_address_line1: Option<String>,
    pub billing_address_line2: Option<String>,
    pub billing_city: Option<String>,
    pub billing_postal_code: Option<String>,
    pub billing_country: Option<String>,

    #[validate(length(min = 1, message = "Metoda płatności jest wymagana."))]
    pub payment_method: String,

    #[validate(length(min = 1, message = "Metoda dostawy jest wymagana."))]
    pub shipping_method_key: String, // np. "inpost", "poczta"}
}

#[derive(Debug, PartialEq, Clone, Display, EnumIter)]
pub enum PaginationItem {
    Page(i64),
    Dots,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrderWithCustomerInfo {
    #[sqlx(flatten)]
    pub order: Order,
    pub customer_email: Option<String>,
}

// Funkcja deserializująca i64 ze stringa lub liczby
fn _deserialize_i64_from_string_or_number<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    struct I64Visitor;

    impl<'de> Visitor<'de> for I64Visitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an integer or a string representing an integer")
        }

        fn visit_i64<E>(self, value: i64) -> Result<i64, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_u64<E>(self, value: u64) -> Result<i64, E>
        where
            E: de::Error,
        {
            i64::try_from(value)
                .map_err(|_| de::Error::invalid_value(Unexpected::Unsigned(value), &self))
        }

        fn visit_str<E>(self, value: &str) -> Result<i64, E>
        where
            E: de::Error,
        {
            i64::from_str(value)
                .map_err(|_| de::Error::invalid_value(Unexpected::Str(value), &self))
        }
    }
    deserializer.deserialize_any(I64Visitor)
}
