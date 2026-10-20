//! Uploading rides to training platforms (R4, R29) behind the [`ActivityUploader`] plugin
//! interface. Each platform decides what it keeps about a rider's account (an API key, the
//! tokens of a login) as [`Credentials`]; ORQWA stores them per rider as given and hands them
//! back for every upload, so a new platform needs no change outside its uploader. Platforms
//! the rider logs in to on their website get the rider's [`Browser`].

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// The future of an uploader's operation, boxed so that the uploaders of all platforms fit
/// one list.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What a platform keeps about a rider's account, by name.
pub type Credentials = BTreeMap<String, String>;

/// A ride to upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    /// The FIT file.
    pub fit: Vec<u8>,
    /// Its name on the platform (R50): FIT files carry none.
    pub name: String,
    /// ORQWA's own id for it, so a platform can tell a second upload of the same ride.
    pub external_id: String,
}

/// Where an upload landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uploaded {
    /// The platform's id for the activity.
    pub id: String,
    /// Its page on the platform.
    pub url: String,
}

/// A rider's connected account on a platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Who is connected, as the platform knows them, for the rider to recognise the account.
    pub label: String,
    /// What the platform needs for uploads.
    pub credentials: Credentials,
}

/// A workout on the rider's calendar at a platform that holds their plan, for one day (R81).
/// The plan stays the platform's: ORQWA reads the day's workouts and rides one, and keeps
/// nothing of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWorkout {
    /// The platform's id of the calendar entry.
    pub id: String,
    /// Its name on the calendar.
    pub name: String,
    /// The workout as a ZWO file, for ORQWA's parser; `None` for a planned ride without steps.
    pub zwo: Option<Vec<u8>>,
}

/// Something the rider types in to connect an account, e.g. an API key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// Its key in the [`Credentials`] entered.
    pub key: &'static str,
    /// Its name for the rider, in English; the front end translates it.
    pub label: &'static str,
    /// Hidden while typed.
    pub secret: bool,
}

/// The rider's browser, for platforms the rider logs in to on their website (OAuth).
pub struct Browser {
    /// Shows the rider a page: the platform's login.
    pub open: Box<dyn Fn(&str) + Send + Sync>,
    /// What the page the browser comes back to says once the account is connected, in the
    /// rider's language.
    pub connected_page: String,
    /// What it says when the login failed or was declined.
    pub failed_page: String,
}

impl fmt::Debug for Browser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Browser")
            .field("connected_page", &self.connected_page)
            .field("failed_page", &self.failed_page)
            .finish_non_exhaustive()
    }
}

/// Connecting or uploading failed. Each carries what went wrong, in words for the rider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadError {
    /// The platform cannot be reached, e.g. offline.
    Unreachable(String),
    /// The platform no longer accepts the account: the rider has to connect it again.
    Unauthorized(String),
    /// The platform turned the request down, or the login did not finish.
    Rejected(String),
    /// The platform disabled the rider's API application; it must be reactivated.
    InactiveApplication(String),
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(why)
            | Self::Unauthorized(why)
            | Self::Rejected(why)
            | Self::InactiveApplication(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for UploadError {}

/// A training platform rides can be uploaded to (R4, R29).
pub trait ActivityUploader: Send + Sync {
    /// The platform's id, as rides and accounts are stored by, e.g. `intervals`.
    fn id(&self) -> &'static str;

    /// Its name for the rider, e.g. "intervals.icu".
    fn name(&self) -> &'static str;

    /// What the rider types in to connect, in order.
    fn fields(&self) -> &'static [Field];

    /// How to come by what [`Self::fields`] asks for, in English; the front end translates it.
    fn help(&self) -> &'static str;

    /// The page [`Self::help`] refers to.
    fn help_url(&self) -> &'static str;

    /// Connects the rider's account with the fields they `entered`, checking them with the
    /// platform and logging in through `browser` where the platform asks for that.
    fn connect(
        &self,
        entered: Credentials,
        browser: Browser,
    ) -> BoxFuture<'_, Result<Account, UploadError>>;

    /// Uploads `activity` to the account with `credentials`. An activity the platform has
    /// already counts as uploaded. Credentials that change on the way, such as a renewed
    /// token, are changed in place for the caller to store, also when the upload then fails.
    fn upload<'a>(
        &'a self,
        credentials: &'a mut Credentials,
        activity: &'a Activity,
    ) -> BoxFuture<'a, Result<Uploaded, UploadError>>;

    /// Withdraws ORQWA's access to the account with `credentials` where the platform keeps a
    /// grant for it, as the rider disconnects it; best effort.
    fn disconnect<'a>(&'a self, credentials: &'a Credentials) -> BoxFuture<'a, ()> {
        let _ = credentials;
        Box::pin(async {})
    }

    /// Whether the platform keeps the rider's training plan, so ORQWA can ask for a day's
    /// workouts (R81). Most only take rides.
    fn holds_plan(&self) -> bool {
        false
    }

    /// The workouts on the rider's calendar for `day` (`YYYY-MM-DD`, the rider's local date),
    /// in calendar order (R81); none unless [`Self::holds_plan`].
    fn planned<'a>(
        &'a self,
        credentials: &'a mut Credentials,
        day: &'a str,
    ) -> BoxFuture<'a, Result<Vec<PlannedWorkout>, UploadError>> {
        let _ = (credentials, day);
        Box::pin(async { Ok(Vec::new()) })
    }
}
