//! Interrupts

pub use bare_metal::{CriticalSection, Mutex, Nr};

/// Trait for enums of external interrupt numbers.
///
/// This trait should be implemented by a peripheral access crate (PAC)
/// on its enum of available external interrupts for a specific device.
/// Each variant must convert to a u16 of its interrupt number,
/// which is its exception number - 16.
///
/// # Safety
///
/// This trait must only be implemented on enums of device interrupts. Each
/// enum variant must represent a distinct value (no duplicates are permitted),
/// and must always return the same value (do not change at runtime).
///
/// These requirements ensure safe nesting of critical sections.
pub unsafe trait InterruptNumber: Copy {
    /// Return the interrupt number associated with this variant.
    ///
    /// See trait documentation for safety requirements.
    fn number(self) -> u16;
}

/// Implement InterruptNumber for the old bare_metal::Nr trait.
/// This implementation is for backwards compatibility only and will be removed in cortex-m 0.8.
unsafe impl<T: Nr + Copy> InterruptNumber for T {
    #[inline]
    fn number(self) -> u16 {
        self.nr() as u16
    }
}

/// Disables all interrupts
#[inline]
pub fn disable() {
    call_asm!(__cpsid());
}

/// Enables all the interrupts
///
/// # Safety
///
/// - Do not call this function inside an `interrupt::free` critical section
#[inline]
pub unsafe fn enable() {
    call_asm!(__cpsie());
}

cfg_if::cfg_if! {
    if #[cfg(feature = "custom-impl")] {
        /// Methods required for a custom critical section implementation.
        ///
        /// This trait is not intended to be used except when implementing a custom critical section.
        ///
        /// Implementations must uphold the contract specified in [`crate::acquire`] and [`crate::release`].
        pub unsafe trait Impl {
            /// Acquire the critical section.
            unsafe fn acquire() -> u8;
            /// Release the critical section.
            unsafe fn release(token: u8);
        }

        /// Set the custom critical section implementation.
        ///
        /// # Example
        ///
        /// ```
        /// struct MyCriticalSection;
        /// critical_section::custom_impl!(MyCriticalSection);
        ///
        /// unsafe impl critical_section::Impl for MyCriticalSection {
        ///     unsafe fn acquire() -> u8 {
        ///         // ...
        ///         # return 0
        ///     }
        ///
        ///     unsafe fn release(token: u8) {
        ///         // ...
        ///     }
        /// }
        ///
        #[macro_export]
        macro_rules! custom_impl {
            ($t: ty) => {
                #[no_mangle]
                unsafe fn _critical_section_acquire() -> u8 {
                    <$t as $crate::interrupt::Impl>::acquire()
                }
                #[no_mangle]
                unsafe fn _critical_section_release(token: u8) {
                    <$t as $crate::interrupt::Impl>::release(token)
                }
            };
        }
    } else {
        #[no_mangle]
        unsafe fn _critical_section_acquire() -> u8 {
            let primask = crate::register::primask::read();
            crate::interrupt::disable();
            primask.is_active() as _
        }

        #[no_mangle]
        unsafe fn _critical_section_release(token: u8) {
            if token != 0 {
                crate::interrupt::enable()
            }
        }
    }
}

/// Execute closure `f` in an interrupt-free context.
///
/// This as also known as a "critical section".
#[inline]
pub fn free<F, R>(f: F) -> R
where
    F: FnOnce(&CriticalSection) -> R,
{

    extern "Rust" {
        fn _critical_section_acquire() -> u8;
        fn _critical_section_release(token: u8);
    }

    let token = unsafe { _critical_section_acquire() };

    let r = f(unsafe { &CriticalSection::new() });

    unsafe { _critical_section_release(token) };

    r
}
