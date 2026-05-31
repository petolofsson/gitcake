use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct Cake {
    pub id: String,
    pub title: String,
    pub created: NaiveDateTime,
    pub owner: Option<String>,
    pub description: Option<String>,
}

#[derive(Default)]
pub struct NewCake {
    pub title: String,
    pub owner: Option<String>,
    pub description: Option<String>,
}
