//! Which three-month window an instant falls in, computed the one way everybody computes it.
//!
//! **Here rather than beside the status lists that use it**, because it is a clock question. If two
//! issuers cut the window at their own midnight, one credential would fall in two windows depending
//! on who was looking — which is the same reason a day is twenty-four closed epochs and not
//! anybody's midnight.
//!
//! # Three months, in UTC, and the UTC is not a detail
//!
//! `SPECS.md §10.2` and `§4.14`. If each issuer cut the window at its own midnight, the same
//! credential would fall in different cohorts depending on who was looking — and the pair
//! *(list, index)* a credential carries would stop meaning one thing.
//!
//! # Why the window has stopped being a privacy decision
//!
//! It was one, and two other rules took the weight off it. With a floor of 131 072 entries the size
//! of a list no longer says how many credentials an issuer has, and with a random index it no
//! longer says how long somebody has been a customer. What is left is a size decision: with
//! credentials lasting one to three years, an issuer keeps between four and twelve live lists of
//! sixteen kilobytes. Irrelevant.
//!
//! And belonging to a cohort reveals nothing the verifier does not already see: the expiry is
//! signed inside the credential, and it reads it anyway.

use crate::{Clock, Epoch};

/// How many months one window covers.
pub const MONTHS: u8 = 3;

/// One window of expiries, named by the year and the quarter it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cohort {
    /// The calendar year, in UTC.
    pub year: i32,
    /// Which quarter of it, from one to four.
    pub quarter: u8,
}

impl Cohort {
    /// The cohort a credential expiring at that epoch belongs to.
    ///
    /// **None before the genesis instant or past what a calendar can hold**, which is not the same
    /// as *the first cohort*: an expiry this cannot place is one no list should be built for.
    #[must_use]
    pub fn of(clock: &Clock, expires: Epoch) -> Option<Self> {
        let when = clock.begins(expires)?.to_offset(time::UtcOffset::UTC);
        Some(Self {
            year: when.year(),
            quarter: (when.month() as u8 - 1) / MONTHS + 1,
        })
    }

    /// The first epoch **after** this window, which is when the whole list may be thrown away.
    ///
    /// Nothing has to be consulted to decide that: every credential the list covered has an expiry
    /// inside the window, signed and unmovable, so all of them are dead.
    #[must_use]
    pub fn over(self, clock: &Clock) -> Option<Epoch> {
        let (year, quarter) = if self.quarter >= 4 {
            (self.year.checked_add(1)?, 1)
        } else {
            (self.year, self.quarter + 1)
        };
        let month = time::Month::try_from((quarter - 1) * MONTHS + 1).ok()?;
        let begins = time::Date::from_calendar_date(year, month, 1)
            .ok()?
            .midnight()
            .assume_utc();
        clock.epoch_at(begins)
    }

    /// Whether this window has passed, and the list covering it can be let go of.
    ///
    /// **The obligation to replicate ends by expiry, with no operation at all** (`SPECS.md §10.2`)
    /// — the same shape `SPECS.md §12.1` already has for a closed entity.
    #[must_use]
    pub fn past(self, clock: &Clock, now: Epoch) -> bool {
        self.over(clock)
            .is_some_and(|ends| now.number() >= ends.number())
    }

    /// How it is written where one has to be named: `2026-Q3`.
    #[must_use]
    pub fn written(self) -> String {
        format!("{}-Q{}", self.year, self.quarter)
    }

    /// One read back from how it was written.
    ///
    /// **The one spelling and no other.** A window written two ways would be two windows to
    /// whoever compares them, and the string is inside what an act's name is hashed over.
    #[must_use]
    pub fn read(written: &str) -> Option<Self> {
        let (year, quarter) = written.split_once("-Q")?;
        // No sign, no padding, no leading noughts: `02026-Q3` and `2026-Q03` are the same window
        // spelled two ways, and one of them is not this spelling.
        let held = Self {
            year: year.parse().ok()?,
            quarter: quarter.parse().ok()?,
        };
        (held.written() == written && (1..=4).contains(&held.quarter)).then_some(held)
    }
}

#[cfg(test)]
mod tests {
    use super::Cohort;
    use crate::{Clock, Epoch};
    use time::{Date, Month, OffsetDateTime};

    /// An instant in UTC, from the calendar.
    fn utc(year: i32, month: u8, day: u8, hour: u8) -> OffsetDateTime {
        Date::from_calendar_date(year, Month::try_from(month).expect("a month"), day)
            .expect("a date")
            .with_hms(hour, 0, 0)
            .expect("a time")
            .assume_utc()
    }

    /// A network opened at the first instant of 2026, so an epoch is an hour from there.
    fn clock() -> Clock {
        Clock::from_genesis(utc(2026, 1, 1, 0))
    }

    /// The epoch an instant falls in on that clock.
    fn at(when: OffsetDateTime) -> Epoch {
        clock().epoch_at(when).expect("after the genesis")
    }

    #[test]
    fn the_window_is_three_months_and_the_edges_are_where_the_calendar_puts_them() {
        let held = clock();
        assert_eq!(
            Cohort::of(&held, at(utc(2026, 1, 1, 0))),
            Some(Cohort {
                year: 2026,
                quarter: 1
            })
        );
        assert_eq!(
            Cohort::of(&held, at(utc(2026, 3, 31, 23))),
            Some(Cohort {
                year: 2026,
                quarter: 1
            }),
            "the last hour of March is still the first quarter"
        );
        assert_eq!(
            Cohort::of(&held, at(utc(2026, 4, 1, 0))),
            Some(Cohort {
                year: 2026,
                quarter: 2
            })
        );
        assert_eq!(
            Cohort::of(&held, at(utc(2026, 12, 31, 23))),
            Some(Cohort {
                year: 2026,
                quarter: 4
            })
        );
    }

    #[test]
    fn a_cohort_is_over_the_moment_its_window_ends_and_not_before() {
        // **What lets a list be thrown away rather than pruned.** Everything it covered has an
        // expiry inside the window, signed inside the credential and unmovable.
        let held = clock();
        let first = Cohort {
            year: 2026,
            quarter: 1,
        };
        let ends = first.over(&held).expect("a calendar");
        assert_eq!(ends, at(utc(2026, 4, 1, 0)));
        assert!(!first.past(&held, at(utc(2026, 3, 31, 23))));
        assert!(first.past(&held, ends));
    }

    #[test]
    fn the_fourth_quarter_rolls_into_the_next_year() {
        let held = clock();
        let last = Cohort {
            year: 2026,
            quarter: 4,
        };
        assert_eq!(last.over(&held), Some(at(utc(2027, 1, 1, 0))));
    }

    #[test]
    fn a_window_reads_back_as_itself_and_nothing_else_does() {
        let held = Cohort {
            year: 2026,
            quarter: 3,
        };
        assert_eq!(Cohort::read(&held.written()), Some(held));
        assert_eq!(Cohort::read("2026-Q0"), None);
        assert_eq!(Cohort::read("2026-Q5"), None);
        assert_eq!(Cohort::read("2026-Q03"), None, "one spelling and no other");
        assert_eq!(Cohort::read("02026-Q3"), None);
        assert_eq!(Cohort::read("2026Q3"), None);
        assert_eq!(Cohort::read(""), None);
    }

    #[test]
    fn an_expiry_before_the_network_existed_is_placed_nowhere() {
        // Not *the first cohort*: an expiry this cannot place is one no list should be built for.
        let held = Clock::from_genesis(utc(2026, 6, 1, 0));
        assert_eq!(
            Cohort::of(&held, Epoch::GENESIS)
                .map(Cohort::written)
                .as_deref(),
            Some("2026-Q2")
        );
        assert_eq!(
            Cohort {
                year: 2026,
                quarter: 1
            }
            .over(&held),
            None,
            "a window that ended before the genesis has no epoch on this clock"
        );
    }
}
