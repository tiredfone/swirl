#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Era {
    E2012,
    E2018,
}

impl Era {
    pub fn year(self) -> u32 {
        match self {
            Era::E2012 => 2012,
            Era::E2018 => 2018,
        }
    }

    pub fn scheme(self) -> &'static str {
        match self {
            Era::E2012 => "solar-2012",
            Era::E2018 => "solar-2018",
        }
    }

    pub fn launcher_name(self) -> String {
        format!("SolarPlayer{}Launcher{}", self.year(), std::env::consts::EXE_SUFFIX)
    }

    pub fn client_name(self) -> String {
        format!("SolarPlayer{}", std::env::consts::EXE_SUFFIX)
    }

    pub fn display(self) -> String {
        format!("Solar {}", self.year())
    }

    pub fn window_size(self) -> (usize, usize) {
        match self {
            Era::E2012 => (420, 210),
            Era::E2018 => (380, 250),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Connecting,
    Checking,
    Downloading,
    Installing,
    Configuring,
    Starting,
    Done,
    Error,
}

impl Phase {
    pub fn parse(s: &str) -> Option<Phase> {
        Some(match s {
            "connecting" => Phase::Connecting,
            "checking" => Phase::Checking,
            "downloading" => Phase::Downloading,
            "installing" => Phase::Installing,
            "configuring" => Phase::Configuring,
            "starting" => Phase::Starting,
            "done" => Phase::Done,
            "error" => Phase::Error,
            _ => return None,
        })
    }

    pub fn text(self, era: Era, percent: Option<u32>, installed_only: bool) -> String {
        let pct = percent.map(|p| format!(" {}%", p)).unwrap_or_default();
        match era {
            Era::E2012 => match self {
                Phase::Connecting => "Connecting to Solar...".into(),
                Phase::Checking => "Looking for updates...".into(),
                Phase::Downloading => format!("Downloading Solar{}", pct),
                Phase::Installing => "Setting up files...".into(),
                Phase::Configuring => "Preparing Solar...".into(),
                Phase::Starting => "Starting Solar...".into(),
                Phase::Done if installed_only => "Solar 2012 is installed.".into(),
                Phase::Done => "Starting Solar...".into(),
                Phase::Error => "Solar could not start".into(),
            },
            Era::E2018 => match self {
                Phase::Connecting => "Getting ready".into(),
                Phase::Checking => "Checking for the newest Solar".into(),
                Phase::Downloading => format!("Downloading{}", pct),
                Phase::Installing => "Installing".into(),
                Phase::Configuring => "Almost there".into(),
                Phase::Starting => "Solar is starting".into(),
                Phase::Done if installed_only => "Solar is installed".into(),
                Phase::Done => "Solar is starting".into(),
                Phase::Error => "Something went wrong".into(),
            },
        }
    }
}
