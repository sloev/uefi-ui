//! Calendar date field (year / month / day) — composite keyboard widget (no drawing).

use crate::input::{Key, KeyEvent};

/// Proleptic Gregorian calendar helpers (host and UEFI).
#[inline]
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Days in `month` (1–12) for `year`.
#[inline]
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 31,
    }
}

/// Which sub-field has focus for Left/Right/Tab navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateField {
    Year,
    Month,
    Day,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateSelectAction {
    /// Any of year / month / day changed.
    Changed,
}

/// Single date value with keyboard-driven editing (Tab / arrows move focus; Up/Down adjust).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateSelect {
    pub year: i32,
    /// 1–12
    pub month: u8,
    /// 1–31 (clamped to the real calendar when month/year change)
    pub day: u8,
    pub focus: DateField,
    pub min_year: i32,
    pub max_year: i32,
}

impl DateSelect {
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        let mut s = Self {
            year,
            month: month.clamp(1, 12),
            day,
            focus: DateField::Day,
            min_year: 1970,
            max_year: 2100,
        };
        s.normalize_day();
        s
    }

    fn normalize_day(&mut self) {
        let dmax = days_in_month(self.year, self.month);
        self.day = self.day.clamp(1, dmax);
    }

    fn next_focus(&mut self) {
        self.focus = match self.focus {
            DateField::Year => DateField::Month,
            DateField::Month => DateField::Day,
            DateField::Day => DateField::Year,
        };
    }

    fn prev_focus(&mut self) {
        self.focus = match self.focus {
            DateField::Year => DateField::Day,
            DateField::Month => DateField::Year,
            DateField::Day => DateField::Month,
        };
    }

    fn bump_year(&mut self, delta: i32) -> bool {
        let ny = (self.year + delta).clamp(self.min_year, self.max_year);
        if ny == self.year {
            return false;
        }
        self.year = ny;
        self.normalize_day();
        true
    }

    fn bump_month(&mut self, delta: i32) -> bool {
        let mut m = self.month as i32 - 1;
        let mut y = self.year;
        m += delta;
        while m < 0 {
            m += 12;
            y -= 1;
        }
        while m >= 12 {
            m -= 12;
            y += 1;
        }
        if y < self.min_year || y > self.max_year {
            return false;
        }
        self.year = y;
        self.month = (m + 1) as u8;
        self.normalize_day();
        true
    }

    fn bump_day(&mut self, delta: i32) -> bool {
        let dmax = days_in_month(self.year, self.month) as i32;
        let mut d = self.day as i32 + delta;
        if d < 1 {
            d = dmax;
        } else if d > dmax {
            d = 1;
        }
        let nd = d as u8;
        if nd == self.day {
            return false;
        }
        self.day = nd;
        true
    }

    /// Up/Down adjust the focused field; Tab / Left / Right move focus; Enter/Escape return `None`.
    pub fn apply_key_event(&mut self, ev: &KeyEvent) -> Option<DateSelectAction> {
        let key = ev.key;
        match key {
            Key::Tab => {
                self.next_focus();
                None
            }
            Key::Left => {
                self.prev_focus();
                None
            }
            Key::Right => {
                self.next_focus();
                None
            }
            Key::Up => {
                let changed = match self.focus {
                    DateField::Year => self.bump_year(1),
                    DateField::Month => self.bump_month(1),
                    DateField::Day => self.bump_day(1),
                };
                changed.then_some(DateSelectAction::Changed)
            }
            Key::Down => {
                let changed = match self.focus {
                    DateField::Year => self.bump_year(-1),
                    DateField::Month => self.bump_month(-1),
                    DateField::Day => self.bump_day(-1),
                };
                changed.then_some(DateSelectAction::Changed)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_february() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
    }

    #[test]
    fn feb29_clamps_to_28_when_not_leap() {
        let d = DateSelect::new(2023, 2, 29);
        assert_eq!(d.day, 28);
    }

    #[test]
    fn month_bump_crosses_year() {
        let mut d = DateSelect::new(2023, 12, 15);
        d.focus = DateField::Month;
        assert_eq!(d.apply_key_event(&KeyEvent::new(Key::Up)), Some(DateSelectAction::Changed));
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 1);
    }
}
