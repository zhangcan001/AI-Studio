use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductionReviewStatus {
    Unreviewed,
    Approved,
    Starred,
    Regenerate,
    Rejected,
}

impl ProductionReviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unreviewed => "UNREVIEWED",
            Self::Approved => "APPROVED",
            Self::Starred => "STARRED",
            Self::Regenerate => "REGENERATE",
            Self::Rejected => "REJECTED",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ProductionReviewDomainError> {
        match value {
            "UNREVIEWED" => Ok(Self::Unreviewed),
            "APPROVED" => Ok(Self::Approved),
            "STARRED" => Ok(Self::Starred),
            "REGENERATE" => Ok(Self::Regenerate),
            "REJECTED" => Ok(Self::Rejected),
            other => Err(ProductionReviewDomainError::InvalidStatus(other.to_owned())),
        }
    }

    pub fn is_accepted(self) -> bool {
        matches!(self, Self::Approved | Self::Starred)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductionReviewDomainError {
    InvalidStatus(String),
}

impl fmt::Display for ProductionReviewDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStatus(value) => {
                write!(formatter, "invalid production review status: {value}")
            }
        }
    }
}

impl Error for ProductionReviewDomainError {}

#[cfg(test)]
mod tests {
    use super::ProductionReviewStatus;

    #[test]
    fn review_status_round_trips() {
        for status in [
            ProductionReviewStatus::Unreviewed,
            ProductionReviewStatus::Approved,
            ProductionReviewStatus::Starred,
            ProductionReviewStatus::Regenerate,
            ProductionReviewStatus::Rejected,
        ] {
            assert_eq!(
                ProductionReviewStatus::parse(status.as_str()).unwrap(),
                status
            );
        }
    }
}
