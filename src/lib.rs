//! Tool to generate Peripheral Access Crates from SVD files
//!
//! This tool has a very different approach compared to `svd2rust` because the requirement are different and are quite similar to [chiptool](https://github.com/embassy-rs/chiptool).
//!
//! ## Major Requirements
//!
//! - Register access should be unsafe because we consider it as C FFI and many times theres HW undefined behavior that can be solved
//!   only at driver level. (e.g. sometimes also the order of writing register bitfields is important.
//!   discussion on this topic available here <https://github.com/rust-embedded/svd2rust/issues/714>).
//! - No ownership because owned registers are an obstacle to writing low level drivers (LLD). Anyway writing
//!   LLDs requires a lot of unsafe code and ownership makes it more complex to access registers from interrupts
//!   and other threads. LLDs shall present safe APIs because only they can implement all logic for a safe usage of peripherals.
//!   Moreover for many peripherals the splitting of peripheral is smaller unit is not obvious and depends on use cases.
//! - Support [tracing](#tracing) of register accesses and additionally mocking of registers on non-embedded devices through
//!   external libraries. This allows the execution unit tests for code that uses the generated libraries on non-embedded devices.
//! - No macros. Absence of macros make easier the debugging.
//! - PAC shall have 0 dependencies to any other crates.
//!    - Exception: `--target=cortex-m`. In this case the generated PAC has some dependencies in order to be usable in ARM Cortex Rust ecosystem.
//! - Use associated constant instead of `Enum` for bitfield values so user can easily create new values.
//!   Enumeration constrains the possible value of a bitfield but many times the SVD
//!   enum description has missing enumeration values. There are multiple reasons:
//!   * Too many values for documentation/SVD.
//!   * Valid values depends on other register values or condition so user documentation writer could decided to not describe in table.
//!   * Lazyness of user manual writer. (Sorry but it sometimes happens ;-))
//!
//! # Known Limitations
//!
//! * Inheritance via `derivedFrom` attribute is presently not supported for bitfields declaration.
//!   Moreover in case that parent is an element of an array only the first element is supported.
//! * `resetMask` tag is ignored
//! * `protection` tag is ignored
//! * `writeConstraint` tag is ignored
//! * `modifiedWriteValues` tag is ignored
//! * `readAction` tag is ignored
//! * `headerEnumName` tag is ignored
//! * in `enumeratedValue` only `value` tag is supported. No support for _don't care bits_ and `isDefault` tag
//!
//! # How to install & prerequisite
//!
//! ```bash
//! cargo install cargo-svd2pac
//! ```
//!
//! if automatic code formatting is desired install `rustfmt`.
//!
//! ```bash
//! rustup component add rustfmt
//! ```
//!
//! # How to use the tool
//!
//! Get a full overview for all cli flags:
//! ```bash
//! svd2pac -h
//! ```
//!
//! Generate PAC without any platform specific code:
//! ```bash
//! svd2pac <your_svd_file> <target directory>
//! ```
//!
//! To generated Aurix PACs use:
//! ```bash
//! svd2pac --target aurix <your_svd_file> <target directory>
//! ```
//!
//! By default `svd2pac` performs strict validation of svd files.
//!
//! It is possible to relax or disable svd validation by using option `--svd-validation-level`
//! ```bash
//! svd2pac --svd-validation-level weak <your_svd_file> <target directory>
//! ```
//! ## Notable CLI flags
//!
//! ---
//! ### Select target :`--target` option
//! This option allows to have target specific code generation
//!
//! #### `--target=generic`
//! This target allows generation of generic code that is independent from any architecture.
//! It ignores  nvicPrioBits, fpuPresent,mpuPresent, vendorSystickConfig attributes and interrupt tag.
//!
//! #### `--target=aurix`
//!
//! Generate the PAC with Aurix platform specific `lmst` instruction support in addition to
//! normal `read/write` instructions.
//!
//! #### `--target=cortex-m`
//!
//! The purpose of this option is generating a PAC that can be used with common cortex-m framework as RTIC.
//! Developer can use CPU register with same API generated by `svd2rust` but for peripheral he shall use the API of `svd2pac`
//! In this way he can reuse the code related to CPU and develop peripheral driver using `svd2pac` style.
//!
//! Extra feature compared to `generic` target
//!
//! - Re-export of cortex-m core peripherals
//! - Peripherals type but now it is possible to call Peripheral::take without limitations.
//! - Interrupt table
//!---
//! ### Enable register mocking: `--tracing` option
//! Enable with the `--tracing` cli flag.
//! Generate the PAC with a non-default feature flag to allow for tracing reads/writes, [see below](#tracing-feature)
//!
//! # How to use the generated code
//!
//! The generator outputs a complete crate into the provided folder.
//! In the generated PACs all peripherals modules are gated by a feature and therefore by default no peripheral modules is compiled.
//! This is speed-up the compilation process. The `features=["all"]` enable the compilations of all modules.
//!
//! ## Naming
//!
//! Some examples showing naming/case, given the timer module in `test_svd/simple.xml`:
//! - `TIMER` instance of a module struct for a peripheral called "timer"
//! - `timer::Timer` type of the module instance above
//! - `TIMER::bitfield_reg()` access function for a register
//! - `timer::bitfield_reg` module containing bitfield structs for the "BITFIELD_REG" register
//! - `timer::bitfield_reg::Run` module containing enumeration values for the "RUN" bitfield
//! - `timer::bitfield_reg::Run::RUNNING` bitfield value constant
//!
//! ## Examples
//!
//! >**Note**
//! >
//! >The following examples are based on the `test_svd/simple.xml` svd used for testing.
//! >In this example we mostly use a `TIMER` module with a few registers, among them:
//! >- `SR` a status register that is mostly read-only
//! >- `BITFIELD_REG` which is a register with multiple bitfields
//! >- `NONBITFIELD_REG`, a register without bitfields
//!
//! ### Read
//!
//! A register is read using the `.read()` function. It returns a struct with convenient functions to access
//! bitfield values. Each bitfield is represented by a struct that is optimized away, the actual values can
//! be retrieved by calling `.get()`
//!
//! ```rust,ignore
//! use test_pac::{timer, TIMER};
//!
//! // Read register `SR` and check `RUN` bitfield
//! let status = unsafe { TIMER.sr().read() };
//! if status.run().get() == timer::sr::Run::RUNNING { /* do something */ }
//!
//! // Access bitfield directly inline
//! while unsafe { TIMER.sr().read().run().get() == timer::sr::Run::RUNNING } { /* ... */ }
//!
//! // Check bitfield with enumeration
//! // (r# as prefix must be used here since `match` is a rust keyword)
//! match unsafe { TIMER.sr().read().r#match().get() } {
//!     timer::sr::Match::NO_MATCH => (),
//!     timer::sr::Match::MATCH_HIT => (),
//!     // since .get() returns a struct, match does not recognize
//!     // an exhaustive match and a wildcard is needed
//!     _ => panic!("impossible"),
//! }
//!
//! // a register might not have a bitfield at all, then we access the value directly
//! let numeric_value = unsafe { TIMER.prescale_rd().read() };
//! ```
//!
//! ### Modify (read/modify/write)
//!
//! The `modify` function takes a closure/function that is passed to the current register value.
//! The closure must modify the passed value and return the value to be written.
//!
//! ```rust,ignore
//! use test_pac::{timer, TIMER};
//!
//! // read `BITFIELD_REG` register, set `BoolRw` to true, `BitfieldRw` to 0x3,
//! // then write back to register
//! unsafe {
//!     TIMER
//!         .bitfield_reg()
//!         .modify(|r| r.boolrw().set(true).bitfieldrw().set(0x3))
//! }
//!
//! // write with dynamic value and enum
//! let x: bool = get_some_bool_value();
//! unsafe {
//!     TIMER.bitfield_reg().modify(|r| {
//!         r.bitfieldenumerated()
//!             .set(timer::bitfield_reg::BitfieldEnumerated::GPIOA_6)
//!             .boolrw()
//!             .set(x)
//!     })
//! }
//! ```
//! > Note: The register is not modified when the `set()` function is called. `set()` modifies the value
//!         stored in the CPU and returns the modified struct. The register is only written once with
//!         the value returned by the closure.
//!
//! > Note: `modify()`, due to doing a read and write with modification of read data in between is not
//!         atomic and can be subject to race conditions and may be interrupted by an interrupt.
//!
//! ### Write
//!
//! A register can be written with an instance of the appropriate struct. The struct instance can be obtained
//! from a read by calling `.default()` (to start off with the register default value) or from a previous
//! register read/write.
//!
//! ```rust,ignore
//! use test_pac::{timer, TIMER};
//!
//! // start with default value, configure some stuff and write to
//! // register.
//! let reg = timer::BitfieldReg::default()
//!     .bitfieldrw()
//!     .set(1)
//!     .boolw()
//!     .set(true);
//! unsafe { TIMER.bitfield_reg().write(reg) };
//!
//! /* do some other initialization */
//!
//! // set `BoolRw` in addition to the settings before and write that
//! // note that .set() returns a new instance of the BitfieldReg struct
//! // with the old being consumed
//! // additional changes could be chained after .set() as above
//! let reg = reg.boolrw().set(true);
//! unsafe { TIMER.bitfield_reg().write(reg) };
//! ```
//!
//! ### Initialization & write-only registers
//!
//! `.init()` allows for the same functionality as `.write()`, but it is limited to start with the register
//! default value. It can also be used as a shorthand for write-only registers.
//!
//! The closure passed to the `.init()` function gets the default value as input and writes back
//! the return value of the closure to the register.
//!
//! ```rust,ignore
//! use test_pac::{timer, TIMER};
//!
//! // do some initializations, write `BoolW` and `BoolRW` with given values,
//! // write others with defaults
//! unsafe {
//!     TIMER
//!         .bitfield_reg()
//!         .init(|r| r.boolw().set(true).boolrw().set(false))
//! }
//!
//! // use init also for write-only registers
//! unsafe {
//!     TIMER.int().init(|r| {
//!         r.en()
//!             .set(timer::int::En::ENABLE)
//!             .mode()
//!             .set(timer::int::Mode::OVERFLOW)
//!     })
//! };
//! ```
//! ### Combine all the things
//! Especially the read and write functionality can be combined, e.g.
//!
//! ```rust,ignore
//! let status = unsafe { TIMER.bitfield_reg().read() };
//! if status.boolr().get() {
//!     let modified = status.boolrw().set(true);
//!     unsafe { TIMER.bitfield_reg().write(modified) }
//! }
//! ```
//!
//! ### Raw access
//! For use cases like logging, initializing from a table, etc. it is
//! possible to read/write registers as plain integers.
//!
//! ```rust,ignore
//! // get register value as integer value
//! let to_log = unsafe { TIMER.sr().read().get_raw() };
//!
//! // write register with integer value, e.g. read from table
//! unsafe { TIMER.bitfield_reg().modify(|r| r.set_raw(0x1234)) };
//! ```
//!
//! ### Modify Atomic (only Aurix)
//! This function is available only for Aurix microcontrollers. It uses the  `ldmst` instruction
//! to read-modify-write a value in a register. This instruction blocks the bus until the end of
//! the transaction. Therefore it affects the other masters on the bus.
//!
//! ```rust,ignore
//! use test_pac::{timer, TIMER};
//! TIMER.bitfield_reg().modify_atomic(|f| {
//!     f.bitfieldenumerated()
//!         .set(timer::bitfield_reg::BitfieldEnumerated::GPIOA_0)
//!         .bitfieldw()
//!         .set(3)
//! });
//! ```
//! Code generation for Aurix is enabled using `--target aurix `
//!
//! ### Array of peripherals
//!
//! SVD arrays of peripherals are modeled using Rust arrays.
//!
//! ```rust,ignore
//! use test_pac::{UART,uart};
//! for peri in UART {
//!     unsafe {
//!         peri.reg16bitenum().modify(|r| {
//!             r.bitfield9bitsenum()
//!                 .set(uart::reg16bitenum::Bitfield9BitsEnum::VAL_0)
//!         })
//!     };
//! }
//! ```
//!
//! ### Array of registers
//! Arrays of registers are modeled as an array of register structs in the module.
//!
//! ```rust,ignore
//! use test_pac::*;
//! let reg_array = TIMER.arrayreg();
//! for reg in reg_array {
//!     let reg_val = unsafe { reg.read() };
//!     let old_val = reg_val.get();
//!     unsafe { reg.write(reg_val.set(old_val + 1)) };
//! }
//! ```
//!
//! ### Array of bitfields
//! Arrays of bitfields are modeled as an array of bitfield structs in the register.
//!
//! ```rust,ignore
//!  let mut reg_value = unsafe { TIMER.bitfield_reg().read() };
//!  for x in 0..2 {
//!     reg_value = reg_value.fieldarray(x).set(timer::bitfield_reg::FieldArray::FALLING);
//!  }
//!  unsafe { TIMER.bitfield_reg().write(reg_value) };
//! ```
//!
//! ### Write an enumerated bitfield by passing an integer literal
//! The size of value cannot exceed bit field size.
//! Here the associated struct type can be created from the integer,
//! as the `From` trait implementation is available for the bitfield structure.
//!
//! ```rust,ignore
//! use test_pac::{timer, TIMER};
//! unsafe {
//!     TIMER.bitfield_reg().modify(|f| {
//!         f.bitfieldenumerated()
//!             .set(0.into())
//!     });
//! }
//! ```
//!
//! ### Get mask and offset of a bitfield
//! It is possible to get mask and offset of a single bitfield using `mask` and `offset`. The returned mask is aligned to the LSB and not shifted (i.e. a 3-bit wide field has a mask of `0x7`, independent of position of the field).
//! ```rust,ignore
//!  use test_pac::{timer, TIMER};
//!  unsafe { 
//!     let register_bitfield = TIMER.bitfield_reg().read().bitfieldr();
//!     let _offset = register_bitfield.offset();
//!     let _mask = register_bitfield.mask();
//! }
//! ```
//!
//! # Tracing feature
//! When generating the PAC with the `--tracing` cli-flag, the PAC is generated with
//! an optional feature flag `tracing`. Enabling the feature provides the following
//! additional functionalities:
//! - an interface where register accesses can be piped though, enabling
//!   developers to log accesses to registers or even mock registers outright.
//!   An implementaion of that interface is provided by [`regmock-rs`](https://github.com/Infineon/regmock-rs).
//! - a special `RegisterValue` trait that allows constructing values of
//!   registers from integers.
//! - an additional `insanely_unsafe` module which allows reading and writing, write-only and
//!   read-only registers (intended for mocking state in tests).
//! - an additional `reg_name` module that contains a perfect hash map of physical
//!   addresses to string names of all registers that reside at an address.
//!
//! ## Examples
//! Below, some simple examples on how to use the tracing APIs are shown.
//! For a complete example of how to use the tracing features for
//! e.g. unittesting see the documentation of [`regmock-rs`](https://github.com/Infineon/regmock-rs).
//!
//! ### Construcing a register value from a raw value with tracing
//! When implementing tests using the tracing feature we want to be
//! able to provide arbitrary data during those tests.
//!
//! ```rust,ignore
//! use pac::common::RegisterValue;
//! let value = pac::peripheral::register::new(0xC0FFEE);
//! unsafe{ pac::PERIPHERAL.register().write(value) };
//! ```
//!
//! ### Reading a value from a **write-only** register with tracing
//! Again for testing: in a testcase we need to do the exact opposite
//! of what normal code does, i.e. we need to "write" read-only registers
//! and "read" write-only registers.
//!
//! Tracing provides a backdoor to allow those actions that are not
//! allowed in normal code.
//!
//! ```rust,ignore
//! use pac::tracing::insanely_unsafe;
//! let value = unsafe{ pac::PERIPHERAL.write_only_register().read_write_only() };
//! ```
//!
//! ### Get the names of registers at a specific address
//! For better logging a map of address to name translation is generated/available
//! if tracing is enabled.
//!
//! ```rust,ignore
//! let regs_at_c0ffee = pac::reg_name::reg_name_from_addr(0xC0FFEE);
//! println!("{regs_at_c0ffee:?}");
//! ```
//!
//! # How to use in your `build.rs`
//!
//! It is possible generate the PAC during application build using [`main`] or [`main_parse_arguments`]
//!
//! # Running tests
//!
//! To execute the tests it is required to add as target "thumbv7em-none-eabihf".
//! This can be done using
//! ```bash
//! rustup target add thumbv7em-none-eabihf
//! ```
//!
//! To test the generation of Aurix PAC it is necessary to install Hightec Rust Aurix compiler and select it as default compiler.
//! `build.rs` detects automatically the toolchain and add the configuration option to enable Aurix specific tests.
//!
//! # Credits
//!
//! A small portion of template common.tera is copied from
//! [Link to commit from where code has been copies](https://github.com/embassy-rs/chiptool/commit/4834a9b35982b393beae84a04960c9add886c1f7)

mod rust_gen;
mod svd_util;
use crate::rust_gen::{generate_rust_package, GenPkgSettings};
use clap::{Parser, ValueEnum};
use env_logger::Env;
use log::{error, info, warn};
use serde::Serialize;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum SvdValidationLevel {
    Disabled,
    Weak,
    Strict,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Serialize)]
pub enum Target {
    /// Only generic access to registers. No support for interrupt vector and NVIC priority bits.
    Generic,
    /// Generic access to register + atomic read modify store of register values. Dependant on tricore crate.
    Aurix,
    /// Support for interrupt vector and NVIC priority bits. Compatible with existing cortex-m-rt crate
    CortexM,
}

/// Generate peripheral access crate from SVD file
#[derive(Parser, Debug)]
#[command(author, version=env!("CARGO_PKG_VERSION"), about="Tool to generate peripheral access crate from SVD file", long_about = None)]
pub struct Args {
    /// Disable formatting of generated code using rustfmt mainly for debugging
    #[arg(long,value_parser=clap::value_parser!(bool),default_value_t=false)]
    pub disable_rust_fmt: bool,
    /// Register description file
    #[arg(value_parser=clap::value_parser!(PathBuf))]
    pub register_description_file_name: PathBuf,
    /// Destination folder of package
    #[arg(value_parser=clap::value_parser!(PathBuf))]
    pub destination_folder: PathBuf,
    //SVD validation level
    #[arg(long,value_enum,default_value_t=SvdValidationLevel::Weak)]
    pub svd_validation_level: SvdValidationLevel,
    /// Architecture target of the PAC.
    #[arg(long,value_enum,default_value_t=Target::Generic)]
    pub target: Target,
    /// Enable the generation of a PAC with the tracing interface.
    #[arg(long,value_parser=clap::value_parser!(bool),default_value_t=false)]
    pub tracing: bool,
    /// Define package name in toml. Default is name stored in register description file
    #[arg(long,value_parser=clap::value_parser!(String),default_value=None)]
    pub package_name: Option<String>,
    /// Specify a license file whose content is used instead of one defined in SVD.
    #[arg(long,value_parser=clap::value_parser!(PathBuf),default_value=None)]
    pub license_file: Option<PathBuf>,
}

/// Main function that parses command line parameters after parsing it invoking [`main`]
///
/// # Arguments
///
/// * `args` - List of command line arguments. The first argument should be the path of executable, but it is ignored by this function.
///
/// # Examples
///
/// ```ignore
/// let args = ["", "./test_svd/simple.xml", "./generated_code"];
/// main_parse_arguments(args);
/// ```

pub fn main_parse_arguments<I, T>(args: I)
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    self::main(Args::parse_from(args));
}

/// Convert SVD file to PAC
pub fn main(args: Args) {
    // Use
    let env = Env::default()
        .filter_or("PAL_LOG_LEVEL", "info")
        .write_style_or("PAL_LOG_STYLE", "always");

    // During test cases the logger is already initialized.
    // Just show a warn
    if let Err(error) = env_logger::try_init_from_env(env) {
        warn!("{}", error);
    }

    info!(
        "Reading register description file {}",
        args.register_description_file_name.to_str().unwrap()
    );
    let destination_folder = args.destination_folder;

    if !destination_folder.exists() {
        info!("Create folder {}", &destination_folder.to_str().unwrap());
        if let Err(err) = fs::create_dir_all(&destination_folder) {
            error!("Failed to create destination folder: {}", err);
            panic!("Failed to create folder")
        };
    }

    if let Err(err) = generate_rust_package(
        &args.register_description_file_name,
        &destination_folder,
        GenPkgSettings {
            run_rustfmt: !args.disable_rust_fmt,
            svd_validation_level: args.svd_validation_level,
            target: args.target,
            tracing: args.tracing,
            package_name: args.package_name,
            license_file: args.license_file,
        },
    ) {
        error!("Failed to generate code with err {}", err);
        panic!("Failed to generate code");
    }
}
