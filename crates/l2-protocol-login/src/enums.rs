//! Reason codes the login server sends to the client.
//!
//! Java: `loginserver/enums/{LoginFailReason,PlayFailReason,AccountKickedReason}.java`.
//!
//! Variant names are the Java names without the `REASON_` prefix, in CamelCase (spelling
//! mistakes included). [`name`](LoginFailReason::name) returns the exact Java name.

use l2_net::PacketError;

/// Declares a `#[repr]` enum with its wire code and Java name per variant. Adds `ALL`, `code()`,
/// `name()` and `TryFrom<code>`, which fails with [`PacketError::Invalid`] for unknown codes.
macro_rules! reason_enum {
    (
        $(#[$meta:meta])*
        $name:ident: $repr:ty {
            $($variant:ident = $code:literal => $java:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[repr($repr)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant = $code,)+
        }

        impl $name {
            /// Every variant, in Java declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// The value written on the wire (Java `getCode()`).
            pub const fn code(self) -> $repr {
                self as $repr
            }

            /// The Java enum constant name, e.g. `REASON_USER_OR_PASS_WRONG`.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $java,)+
                }
            }
        }

        impl TryFrom<$repr> for $name {
            type Error = PacketError;

            fn try_from(code: $repr) -> Result<Self, PacketError> {
                match code {
                    $($code => Ok(Self::$variant),)+
                    _ => Err(PacketError::invalid(
                        "reason",
                        format!("unknown {} code {code:#04x}", stringify!($name)),
                    )),
                }
            }
        }
    };
}

reason_enum! {
    /// Reason in `LoginFail` (one byte). Java: `LoginFailReason`.
    LoginFailReason: u8 {
        NoMessage = 0x00 => "REASON_NO_MESSAGE",
        SystemErrorLoginLater = 0x01 => "REASON_SYSTEM_ERROR_LOGIN_LATER",
        UserOrPassWrong = 0x02 => "REASON_USER_OR_PASS_WRONG",
        AccessFailedTryAgainLater = 0x04 => "REASON_ACCESS_FAILED_TRY_AGAIN_LATER",
        AccountInfoIncorrectContactSupport = 0x05 => "REASON_ACCOUNT_INFO_INCORRECT_CONTACT_SUPPORT",
        NotAuthed = 0x06 => "REASON_NOT_AUTHED",
        AccountInUse = 0x07 => "REASON_ACCOUNT_IN_USE",
        Under18YearsKr = 0x0C => "REASON_UNDER_18_YEARS_KR",
        ServerOverloaded = 0x0F => "REASON_SERVER_OVERLOADED",
        ServerMaintenance = 0x10 => "REASON_SERVER_MAINTENANCE",
        TempPassExpired = 0x11 => "REASON_TEMP_PASS_EXPIRED",
        GameTimeExpired = 0x12 => "REASON_GAME_TIME_EXPIRED",
        NoTimeLeft = 0x13 => "REASON_NO_TIME_LEFT",
        SystemError = 0x14 => "REASON_SYSTEM_ERROR",
        AccessFailed = 0x15 => "REASON_ACCESS_FAILED",
        RestrictedIp = 0x16 => "REASON_RESTRICTED_IP",
        WeekUsageFinished = 0x1E => "REASON_WEEK_USAGE_FINISHED",
        SecurityCardNumberInvalid = 0x1F => "REASON_SECURITY_CARD_NUMBER_INVALID",
        AgeNotVerifiedCantLogBeetween10pm6am = 0x20 => "REASON_AGE_NOT_VERIFIED_CANT_LOG_BEETWEEN_10PM_6AM",
        ServerCannotBeAccessedByYourCoupon = 0x21 => "REASON_SERVER_CANNOT_BE_ACCESSED_BY_YOUR_COUPON",
        DualBox = 0x23 => "REASON_DUAL_BOX",
        Inactive = 0x24 => "REASON_INACTIVE",
        UserAgreementRejectedOnWebsite = 0x25 => "REASON_USER_AGREEMENT_REJECTED_ON_WEBSITE",
        GuardianConsentRequired = 0x26 => "REASON_GUARDIAN_CONSENT_REQUIRED",
        UserAgreementDeclinedOrWithdrawlRequest = 0x27 => "REASON_USER_AGREEMENT_DECLINED_OR_WITHDRAWL_REQUEST",
        AccountSuspendedCall = 0x28 => "REASON_ACCOUNT_SUSPENDED_CALL",
        ChangePasswordAndQuizOnWebsite = 0x29 => "REASON_CHANGE_PASSWORD_AND_QUIZ_ON_WEBSITE",
        AlreadyLoggedInto10Accounts = 0x2A => "REASON_ALREADY_LOGGED_INTO_10_ACCOUNTS",
        MasterAccountRestricted = 0x2B => "REASON_MASTER_ACCOUNT_RESTRICTED",
        CertificationFailed = 0x2E => "REASON_CERTIFICATION_FAILED",
        TelephoneCertificationUnavailable = 0x2F => "REASON_TELEPHONE_CERTIFICATION_UNAVAILABLE",
        TelephoneSignalsDelayed = 0x30 => "REASON_TELEPHONE_SIGNALS_DELAYED",
        CertificationFailedLineBusy = 0x31 => "REASON_CERTIFICATION_FAILED_LINE_BUSY",
        CertificationServiceNumberExpiredOrIncorrect = 0x32 => "REASON_CERTIFICATION_SERVICE_NUMBER_EXPIRED_OR_INCORRECT",
        CertificationServiceCurrentlyBeingChecked = 0x33 => "REASON_CERTIFICATION_SERVICE_CURRENTLY_BEING_CHECKED",
        CertificationServiceCantBeUsedHeavyVolume = 0x34 => "REASON_CERTIFICATION_SERVICE_CANT_BE_USED_HEAVY_VOLUME",
        CertificationServiceExpiredGameplayBlocked = 0x35 => "REASON_CERTIFICATION_SERVICE_EXPIRED_GAMEPLAY_BLOCKED",
        CertificationFailed3TimesGameplayBlocked30Min = 0x36 => "REASON_CERTIFICATION_FAILED_3_TIMES_GAMEPLAY_BLOCKED_30_MIN",
        CertificationDailyUseExceeded = 0x37 => "REASON_CERTIFICATION_DAILY_USE_EXCEEDED",
        CertificationUnderwayTryAgainLater = 0x38 => "REASON_CERTIFICATION_UNDERWAY_TRY_AGAIN_LATER",
    }
}

reason_enum! {
    /// Reason in `PlayFail` (one byte). Java: `PlayFailReason`.
    ///
    /// NOTE(java-quirk): same list as [`LoginFailReason`] but without `NotAuthed` (0x06).
    PlayFailReason: u8 {
        NoMessage = 0x00 => "REASON_NO_MESSAGE",
        SystemErrorLoginLater = 0x01 => "REASON_SYSTEM_ERROR_LOGIN_LATER",
        UserOrPassWrong = 0x02 => "REASON_USER_OR_PASS_WRONG",
        AccessFailedTryAgainLater = 0x04 => "REASON_ACCESS_FAILED_TRY_AGAIN_LATER",
        AccountInfoIncorrectContactSupport = 0x05 => "REASON_ACCOUNT_INFO_INCORRECT_CONTACT_SUPPORT",
        AccountInUse = 0x07 => "REASON_ACCOUNT_IN_USE",
        Under18YearsKr = 0x0C => "REASON_UNDER_18_YEARS_KR",
        ServerOverloaded = 0x0F => "REASON_SERVER_OVERLOADED",
        ServerMaintenance = 0x10 => "REASON_SERVER_MAINTENANCE",
        TempPassExpired = 0x11 => "REASON_TEMP_PASS_EXPIRED",
        GameTimeExpired = 0x12 => "REASON_GAME_TIME_EXPIRED",
        NoTimeLeft = 0x13 => "REASON_NO_TIME_LEFT",
        SystemError = 0x14 => "REASON_SYSTEM_ERROR",
        AccessFailed = 0x15 => "REASON_ACCESS_FAILED",
        RestrictedIp = 0x16 => "REASON_RESTRICTED_IP",
        WeekUsageFinished = 0x1E => "REASON_WEEK_USAGE_FINISHED",
        SecurityCardNumberInvalid = 0x1F => "REASON_SECURITY_CARD_NUMBER_INVALID",
        AgeNotVerifiedCantLogBeetween10pm6am = 0x20 => "REASON_AGE_NOT_VERIFIED_CANT_LOG_BEETWEEN_10PM_6AM",
        ServerCannotBeAccessedByYourCoupon = 0x21 => "REASON_SERVER_CANNOT_BE_ACCESSED_BY_YOUR_COUPON",
        DualBox = 0x23 => "REASON_DUAL_BOX",
        Inactive = 0x24 => "REASON_INACTIVE",
        UserAgreementRejectedOnWebsite = 0x25 => "REASON_USER_AGREEMENT_REJECTED_ON_WEBSITE",
        GuardianConsentRequired = 0x26 => "REASON_GUARDIAN_CONSENT_REQUIRED",
        UserAgreementDeclinedOrWithdrawlRequest = 0x27 => "REASON_USER_AGREEMENT_DECLINED_OR_WITHDRAWL_REQUEST",
        AccountSuspendedCall = 0x28 => "REASON_ACCOUNT_SUSPENDED_CALL",
        ChangePasswordAndQuizOnWebsite = 0x29 => "REASON_CHANGE_PASSWORD_AND_QUIZ_ON_WEBSITE",
        AlreadyLoggedInto10Accounts = 0x2A => "REASON_ALREADY_LOGGED_INTO_10_ACCOUNTS",
        MasterAccountRestricted = 0x2B => "REASON_MASTER_ACCOUNT_RESTRICTED",
        CertificationFailed = 0x2E => "REASON_CERTIFICATION_FAILED",
        TelephoneCertificationUnavailable = 0x2F => "REASON_TELEPHONE_CERTIFICATION_UNAVAILABLE",
        TelephoneSignalsDelayed = 0x30 => "REASON_TELEPHONE_SIGNALS_DELAYED",
        CertificationFailedLineBusy = 0x31 => "REASON_CERTIFICATION_FAILED_LINE_BUSY",
        CertificationServiceNumberExpiredOrIncorrect = 0x32 => "REASON_CERTIFICATION_SERVICE_NUMBER_EXPIRED_OR_INCORRECT",
        CertificationServiceCurrentlyBeingChecked = 0x33 => "REASON_CERTIFICATION_SERVICE_CURRENTLY_BEING_CHECKED",
        CertificationServiceCantBeUsedHeavyVolume = 0x34 => "REASON_CERTIFICATION_SERVICE_CANT_BE_USED_HEAVY_VOLUME",
        CertificationServiceExpiredGameplayBlocked = 0x35 => "REASON_CERTIFICATION_SERVICE_EXPIRED_GAMEPLAY_BLOCKED",
        CertificationFailed3TimesGameplayBlocked30Min = 0x36 => "REASON_CERTIFICATION_FAILED_3_TIMES_GAMEPLAY_BLOCKED_30_MIN",
        CertificationDailyUseExceeded = 0x37 => "REASON_CERTIFICATION_DAILY_USE_EXCEEDED",
        CertificationUnderwayTryAgainLater = 0x38 => "REASON_CERTIFICATION_UNDERWAY_TRY_AGAIN_LATER",
    }
}

reason_enum! {
    /// Reason in `AccountKicked` (written as a 32-bit int). Java: `AccountKickedReason`.
    AccountKickedReason: u32 {
        DataStealer = 0x01 => "REASON_DATA_STEALER",
        GenericViolation = 0x08 => "REASON_GENERIC_VIOLATION",
        SevenDaysSuspended = 0x10 => "REASON_7_DAYS_SUSPENDED",
        PermanentlyBanned = 0x20 => "REASON_PERMANENTLY_BANNED",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_match_java() {
        assert_eq!(LoginFailReason::ALL.len(), 40);
        assert_eq!(PlayFailReason::ALL.len(), 39);
        assert_eq!(AccountKickedReason::ALL.len(), 4);
    }

    #[test]
    fn code_round_trip_for_every_variant() {
        for &r in LoginFailReason::ALL {
            assert_eq!(LoginFailReason::try_from(r.code()), Ok(r));
        }
        for &r in PlayFailReason::ALL {
            assert_eq!(PlayFailReason::try_from(r.code()), Ok(r));
        }
        for &r in AccountKickedReason::ALL {
            assert_eq!(AccountKickedReason::try_from(r.code()), Ok(r));
        }
    }

    #[test]
    fn spot_checks() {
        assert_eq!(LoginFailReason::UserOrPassWrong.code(), 0x02);
        assert_eq!(LoginFailReason::AccessFailed.name(), "REASON_ACCESS_FAILED");
        assert_eq!(PlayFailReason::ServerOverloaded.code(), 0x0F);
        assert_eq!(AccountKickedReason::PermanentlyBanned.code(), 0x20);
        assert_eq!(
            AccountKickedReason::SevenDaysSuspended.name(),
            "REASON_7_DAYS_SUSPENDED"
        );
    }

    #[test]
    fn unknown_codes_are_rejected() {
        assert!(LoginFailReason::try_from(0x03).is_err());
        assert!(PlayFailReason::try_from(0x06).is_err(), "NotAuthed is login-only");
        assert!(AccountKickedReason::try_from(0).is_err());
        assert!(AccountKickedReason::try_from(0x0100_0020).is_err());
    }
}
