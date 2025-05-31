// src/models.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use strum_macros::{Display, EnumIter, EnumString};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString, Display)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString, Display)]
#[sqlx(type_name = "product_status")]
#[strum(ascii_case_insensitive)]
pub enum ProductStatus {
    #[strum(serialize = "Dostępny")]
    Available,
    #[strum(serialize = "Zarezerwowany")]
    Reserved,
    #[strum(serialize = "Sprzedany")]
    Sold,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString, Display, EnumIter)]
#[sqlx(type_name = "product_gender")]
pub enum ProductGender {
    Damskie,
    Meskie,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString, Display, EnumIter)]
#[sqlx(type_name = "category_type")]
#[strum(ascii_case_insensitive)]
pub enum Category {
    Koszule,
    Spodnie,
    Sukienki,
    #[strum(serialize = "Spódnice")]
    Spodnice,
    Swetry,
    Bluzy,
    #[strum(serialize = "Kurtki i Żakiety")]
    KurtkiPlaszcze,
    #[strum(serialize = "Marynarki i Płaszcze")]
    MarynarkiZakiety,
    Obuwie,
    Torebki,
    Akcesoria,
    Bielizna,
    #[strum(serialize = "Stroje kąpielowe")]
    StrojeKapielowe,
    Inne,
}

impl Category {
    pub fn as_url_param(&self) -> String {
        match self {
            Category::Koszule => "Koszule".to_string(),
            Category::Spodnie => "Spodnie".to_string(),
            Category::Sukienki => "Sukienki".to_string(),
            Category::Spodnice => "Spodnice".to_string(),
            Category::Swetry => "Swetry".to_string(),
            Category::Bluzy => "Bluzy".to_string(),
            Category::KurtkiPlaszcze => "KurtkiPlaszcze".to_string(),
            Category::MarynarkiZakiety => "MarynarkiZakiety".to_string(),
            Category::Obuwie => "Obuwie".to_string(),
            Category::Torebki => "Torebki".to_string(),
            Category::Akcesoria => "Akcesoria".to_string(),
            Category::Bielizna => "Bielizna".to_string(),
            Category::StrojeKapielowe => "StrojeKapielowe".to_string(),
            Category::Inne => "Inne".to_string(),
        }
    }
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type, Display)]
#[sqlx(type_name = "order_status_enum")]
#[sqlx(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,    // Oczekujące (np. na płatność)
    Processing, // W trakcie realizacji
    Shipped,    // Wysłane
    Delivered,  // Dostarczone
    Cancelled,  // Anulowane
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
    pub user_id: Uuid,
    pub order_date: DateTime<Utc>,
    pub status: OrderStatus,
    pub total_price: i64,

    #[validate(length(min = 1, max = 255))]
    pub shipping_address_line1: String,

    #[validate(length(max = 255))]
    pub shipping_address_line2: String,

    #[validate(length(min = 1, max = 100))]
    pub shipping_city: String,

    #[validate(length(min = 1, max = 20))]
    pub shipping_postal_code: String,

    #[validate(length(min = 1, max = 100))]
    pub shipping_country: String,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

#[derive(Deserialize, Debug)]
pub struct CheckoutFormData {
    pub shipping_first_name: String,
    pub shipping_last_name: String,
    pub shipping_address_line1: String,
    pub shipping_address_line2: Option<String>,
    pub shipping_city: String,
    pub shipping_postal_code: String,
    pub shipping_country: String,
    pub shipping_phone: String,
    pub billing_same_as_shipping: bool,
    pub billing_first_name: Option<String>,
    pub billing_last_name: Option<String>,
    pub billing_address_line1: Option<String>,
    pub billing_address_line2: Option<String>,
    pub billing_city: Option<String>,
    pub billing_postal_code: Option<String>,
    pub billing_country: Option<String>,
    pub payment_method: String,
    pub notes: Option<String>,
}

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
    pub status: ProductStatus, // p.status
    pub images: Vec<String>, // p.images
}
