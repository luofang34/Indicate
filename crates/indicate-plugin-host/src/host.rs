//! Sandboxed [`wasmi`] instance management and fail-closed `ind_draw` execution (`PLUG-01`).

use indicate_instrument_descriptor::{DesignFrame, DrawContext, encode_draw_context};
use indicate_instrument_scene::angular::{MAX_ANGULAR_SCENE_BYTES, validate_angular_layers};
use indicate_instrument_scene::volume::{Extent3D, MAX_VOLUME_SCENE_BYTES, validate_volume_bands};
use indicate_instrument_scene::{MAX_SCENE_BYTES, validate_layers};
use indicate_plugin_abi::{
    PluginDescriptorWire, PluginSpaceWire, SignatureVerifier, TARGET_KIND_ANGULAR_OVERLAY_2D,
    TARGET_KIND_ANGULAR_SPHERE, TARGET_KIND_PLANAR_2D, TARGET_KIND_VOLUME_3D, WASM_ABI_VERSION,
    unpack_indpack,
};
use wasmi::{
    Config, Engine, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TrapCode,
    TypedFunc,
};

use crate::error::PluginHostError;

/// Maximum Wasm linear-memory size permitted in the plugin sandbox (`4` pages = `256 KiB`).
pub const MAX_WASM_MEMORY_PAGES: u64 = 4;

/// Default deterministic instruction fuel budget per draw invocation (`2_000_000`).
pub const DEFAULT_FRAME_FUEL: u64 = 2_000_000;

const WASM_PAGE_BYTES: usize = 65_536;
const MAX_WASM_MEMORY_BYTES: usize = (MAX_WASM_MEMORY_PAGES as usize) * WASM_PAGE_BYTES;
const STARTUP_PROBE_FUEL: u64 = 100_000;
const MIN_SCRATCH_CAP: usize = 512;
const HOST_CTX_BUF_BYTES: usize = 65_536;

type IndDrawFunc = TypedFunc<(i32, i32, i32, i32, i32, i32, i32), i32>;

struct HostState {
    limits: StoreLimits,
}

/// Output buffer target handed to [`LoadedPlugin::draw`].
pub enum PluginDrawTarget<'a> {
    /// Planar 2D render target at `frame`.
    Planar2D {
        /// Requested 2D design frame.
        frame: DesignFrame,
        /// Caller output buffer for validated 2D scene bytes.
        out: &'a mut [u8],
    },
    /// Unit-sphere angular target with optional 2D peripheral overlay buffer.
    AngularSphere {
        /// Caller output buffer for validated angular scene bytes.
        angular_out: &'a mut [u8],
        /// Optional `(frame, out)` buffer for validated 2D overlay bytes.
        overlay_out: Option<(DesignFrame, &'a mut [u8])>,
    },
    /// Bounded 3D metric volume target at `extent_m`.
    Volume3D {
        /// Requested 3D volume dimensions in meters.
        extent_m: Extent3D,
        /// Caller output buffer for validated 3D volume scene bytes.
        out: &'a mut [u8],
    },
}

/// Byte counts and fuel consumption reported by [`LoadedPlugin::draw`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginDrawReport {
    /// Validated primary scene byte count written to `out` / `angular_out`.
    pub primary_bytes: usize,
    /// Validated 2D overlay byte count written to `overlay_out` (`0` when absent).
    pub overlay_bytes: usize,
    /// Deterministic Wasm fuel consumed across this draw call.
    pub fuel_consumed: u64,
}

/// Verified, instantiated `.indpack` Wasm plugin ready for deterministic drawing (`PLUG-01`).
pub struct LoadedPlugin {
    descriptor: PluginDescriptorWire,
    wasm_sha256: [u8; 32],
    store: Store<HostState>,
    memory: Memory,
    ind_draw: IndDrawFunc,
    scratch_ptr: usize,
    scratch_cap: usize,
    frame_fuel: u64,
}

impl core::fmt::Debug for LoadedPlugin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LoadedPlugin")
            .field("id", &self.descriptor.id())
            .field("scratch_ptr", &self.scratch_ptr)
            .field("scratch_cap", &self.scratch_cap)
            .field("frame_fuel", &self.frame_fuel)
            .finish_non_exhaustive()
    }
}

impl LoadedPlugin {
    /// Unpacks, verifies SHA-256 + signature, validates sandbox limits, and instantiates `.indpack`.
    pub fn load(
        indpack_bytes: &[u8],
        verifier: &dyn SignatureVerifier,
    ) -> Result<Self, PluginHostError> {
        let unpacked = unpack_indpack(indpack_bytes, verifier)?;
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, unpacked.wasm_bytes)
            .map_err(|e| PluginHostError::WasmModule(e.to_string()))?;
        if module.imports().next().is_some() {
            return Err(PluginHostError::HostImportsForbidden);
        }
        let limits = StoreLimitsBuilder::new()
            .memory_size(MAX_WASM_MEMORY_BYTES)
            .memories(1)
            .instances(1)
            .tables(2)
            .table_elements(256)
            .build();
        let mut store = Store::new(&engine, HostState { limits });
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(STARTUP_PROBE_FUEL)
            .map_err(|e| PluginHostError::WasmModule(e.to_string()))?;

        let linker = Linker::<HostState>::new(&engine);
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(map_wasmi_err)?;

        let memory = instance
            .get_memory(&store, "memory")
            .ok_or(PluginHostError::MissingExport("memory"))?;
        check_memory_limits(&store, &memory)?;

        let abi_fn: TypedFunc<(), i32> = instance
            .get_typed_func(&store, "ind_abi_version")
            .map_err(|_| PluginHostError::MissingExport("ind_abi_version"))?;
        let ptr_fn: TypedFunc<(), i32> = instance
            .get_typed_func(&store, "ind_scratch_ptr")
            .map_err(|_| PluginHostError::MissingExport("ind_scratch_ptr"))?;
        let cap_fn: TypedFunc<(), i32> = instance
            .get_typed_func(&store, "ind_scratch_cap")
            .map_err(|_| PluginHostError::MissingExport("ind_scratch_cap"))?;
        let ind_draw: IndDrawFunc = instance
            .get_typed_func(&store, "ind_draw")
            .map_err(|_| PluginHostError::MissingExport("ind_draw"))?;

        let abi = abi_fn.call(&mut store, ()).map_err(map_wasmi_err)?;
        if abi != WASM_ABI_VERSION as i32 {
            return Err(PluginHostError::UnsupportedAbiVersion(abi));
        }
        let raw_ptr = ptr_fn.call(&mut store, ()).map_err(map_wasmi_err)?;
        let raw_cap = cap_fn.call(&mut store, ()).map_err(map_wasmi_err)?;
        let (scratch_ptr, scratch_cap) =
            validate_scratch_window(raw_ptr, raw_cap, memory.data_size(&store))?;

        Ok(Self {
            descriptor: unpacked.descriptor,
            wasm_sha256: unpacked.wasm_sha256,
            store,
            memory,
            ind_draw,
            scratch_ptr,
            scratch_cap,
            frame_fuel: DEFAULT_FRAME_FUEL,
        })
    }

    /// Overrides the deterministic per-frame fuel limit (`DEFAULT_FRAME_FUEL` by default).
    #[must_use]
    pub fn with_frame_fuel(mut self, fuel: u64) -> Self {
        self.frame_fuel = fuel;
        self
    }

    /// Validated static plugin descriptor unpacked from `.indpack`.
    #[must_use]
    pub const fn descriptor(&self) -> &PluginDescriptorWire {
        &self.descriptor
    }

    /// Verified SHA-256 digest of the guest Wasm module.
    #[must_use]
    pub const fn wasm_sha256(&self) -> &[u8; 32] {
        &self.wasm_sha256
    }

    /// Encodes `ctx` into guest scratch, executes `ind_draw`, and validates the emitted scene fail-closed.
    pub fn draw(
        &mut self,
        ctx: &DrawContext<'_>,
        target: PluginDrawTarget<'_>,
    ) -> Result<PluginDrawReport, PluginHostError> {
        let mut ctx_buf = vec![0u8; HOST_CTX_BUF_BYTES];
        let ctx_len = encode_draw_context(ctx, &mut ctx_buf)?;
        match (&self.descriptor.space, target) {
            (PluginSpaceWire::Planar2D { .. }, PluginDrawTarget::Planar2D { frame, out }) => {
                if !accepts_planar(&self.descriptor.space, frame) {
                    return Err(PluginHostError::TargetMismatch);
                }
                let (written, fuel) = self.invoke_single(
                    &ctx_buf[..ctx_len],
                    TARGET_KIND_PLANAR_2D,
                    [
                        frame.width.to_bits() as i32,
                        frame.height.to_bits() as i32,
                        0,
                    ],
                    MAX_SCENE_BYTES,
                    out,
                )?;
                let report = validate_layers(&out[..written])?;
                let missing = self.descriptor.required_layers & !u16::from(report.present);
                if missing != 0 {
                    return Err(PluginHostError::MissingRequiredLayers(missing));
                }
                Ok(PluginDrawReport {
                    primary_bytes: written,
                    overlay_bytes: 0,
                    fuel_consumed: fuel,
                })
            }
            (
                PluginSpaceWire::Volume3D {
                    extent_min_m,
                    extent_max_m,
                    ..
                },
                PluginDrawTarget::Volume3D { extent_m, out },
            ) => {
                if !accepts_volume(*extent_min_m, *extent_max_m, extent_m) {
                    return Err(PluginHostError::TargetMismatch);
                }
                let (written, fuel) = self.invoke_single(
                    &ctx_buf[..ctx_len],
                    TARGET_KIND_VOLUME_3D,
                    [
                        extent_m.width_m.to_bits() as i32,
                        extent_m.height_m.to_bits() as i32,
                        extent_m.depth_m.to_bits() as i32,
                    ],
                    MAX_VOLUME_SCENE_BYTES,
                    out,
                )?;
                let report = validate_volume_bands(&out[..written])?;
                let missing = self.descriptor.required_layers & !report.present;
                if missing != 0 {
                    return Err(PluginHostError::MissingRequiredLayers(missing));
                }
                Ok(PluginDrawReport {
                    primary_bytes: written,
                    overlay_bytes: 0,
                    fuel_consumed: fuel,
                })
            }
            (
                PluginSpaceWire::AngularSphere { overlay_frame },
                PluginDrawTarget::AngularSphere {
                    angular_out,
                    overlay_out,
                },
            ) => {
                let ov_expected = *overlay_frame;
                self.draw_angular(&ctx_buf[..ctx_len], ov_expected, angular_out, overlay_out)
            }
            _ => Err(PluginHostError::TargetMismatch),
        }
    }

    fn draw_angular(
        &mut self,
        ctx_bytes: &[u8],
        ov_expected: Option<DesignFrame>,
        angular_out: &mut [u8],
        overlay_out: Option<(DesignFrame, &mut [u8])>,
    ) -> Result<PluginDrawReport, PluginHostError> {
        let (ang_written, ang_fuel) = self.invoke_single(
            ctx_bytes,
            TARGET_KIND_ANGULAR_SPHERE,
            [0, 0, 0],
            MAX_ANGULAR_SCENE_BYTES,
            angular_out,
        )?;
        let ang_rep = validate_angular_layers(&angular_out[..ang_written])?;
        let missing_ang = (self.descriptor.required_layers & 0x00FF) & !u16::from(ang_rep.present);
        if missing_ang != 0 {
            return Err(PluginHostError::MissingRequiredLayers(missing_ang));
        }
        match (ov_expected, overlay_out) {
            (None, None) => Ok(PluginDrawReport {
                primary_bytes: ang_written,
                overlay_bytes: 0,
                fuel_consumed: ang_fuel,
            }),
            (Some(exp_frame), Some((frame, ov_buf))) if exp_frame == frame => {
                let (ov_written, ov_fuel) = self.invoke_single(
                    ctx_bytes,
                    TARGET_KIND_ANGULAR_OVERLAY_2D,
                    [
                        frame.width.to_bits() as i32,
                        frame.height.to_bits() as i32,
                        0,
                    ],
                    MAX_SCENE_BYTES,
                    ov_buf,
                )?;
                let ov_rep = validate_layers(&ov_buf[..ov_written])?;
                let req_ov = (self.descriptor.required_layers >> 8) as u8;
                let missing_ov = req_ov & !ov_rep.present;
                if missing_ov != 0 {
                    return Err(PluginHostError::MissingRequiredLayers(
                        u16::from(missing_ov) << 8,
                    ));
                }
                Ok(PluginDrawReport {
                    primary_bytes: ang_written,
                    overlay_bytes: ov_written,
                    fuel_consumed: ang_fuel.saturating_add(ov_fuel),
                })
            }
            _ => Err(PluginHostError::TargetMismatch),
        }
    }

    fn invoke_single(
        &mut self,
        ctx_bytes: &[u8],
        target_kind: i32,
        dims: [i32; 3],
        max_scene: usize,
        out: &mut [u8],
    ) -> Result<(usize, u64), PluginHostError> {
        let ctx_len = ctx_bytes.len();
        let out_offset = (ctx_len + 7) & !7;
        if out_offset >= self.scratch_cap {
            return Err(PluginHostError::InvalidScratchWindow {
                ptr: self.scratch_ptr as i32,
                cap: self.scratch_cap as i32,
            });
        }
        let guest_cap = (self.scratch_cap - out_offset)
            .min(max_scene)
            .min(out.len());
        if guest_cap == 0 {
            return Err(PluginHostError::SceneOverflow { written: 1, cap: 0 });
        }
        self.memory
            .write(&mut self.store, self.scratch_ptr, ctx_bytes)
            .map_err(|_| PluginHostError::InvalidScratchWindow {
                ptr: self.scratch_ptr as i32,
                cap: self.scratch_cap as i32,
            })?;
        self.store
            .set_fuel(self.frame_fuel)
            .map_err(|e| PluginHostError::WasmModule(e.to_string()))?;
        let ret = self
            .ind_draw
            .call(
                &mut self.store,
                (
                    ctx_len as i32,
                    target_kind,
                    dims[0],
                    dims[1],
                    dims[2],
                    out_offset as i32,
                    guest_cap as i32,
                ),
            )
            .map_err(map_wasmi_err)?;
        check_memory_limits(&self.store, &self.memory)?;
        let remaining = self.store.get_fuel().unwrap_or(0);
        let consumed = self.frame_fuel.saturating_sub(remaining);
        if ret < 0 {
            return Err(PluginHostError::GuestDrawError(ret));
        }
        let written = ret as usize;
        if written > guest_cap {
            return Err(PluginHostError::SceneOverflow {
                written,
                cap: guest_cap,
            });
        }
        let src_start = self.scratch_ptr + out_offset;
        self.memory
            .read(&self.store, src_start, &mut out[..written])
            .map_err(|_| PluginHostError::SceneOverflow {
                written,
                cap: guest_cap,
            })?;
        Ok((written, consumed))
    }
}

fn check_memory_limits(store: &Store<HostState>, memory: &Memory) -> Result<(), PluginHostError> {
    let ty = memory.ty(store);
    if ty.is_64()
        || ty.minimum() > MAX_WASM_MEMORY_PAGES
        || ty.maximum().is_some_and(|m| m > MAX_WASM_MEMORY_PAGES)
        || memory.data_size(store) > MAX_WASM_MEMORY_BYTES
    {
        return Err(PluginHostError::MemoryLimitExceeded);
    }
    Ok(())
}

fn validate_scratch_window(
    ptr: i32,
    cap: i32,
    mem_size: usize,
) -> Result<(usize, usize), PluginHostError> {
    if ptr < 0 || cap < MIN_SCRATCH_CAP as i32 {
        return Err(PluginHostError::InvalidScratchWindow { ptr, cap });
    }
    let uptr = ptr as usize;
    let ucap = cap as usize;
    let end = uptr
        .checked_add(ucap)
        .ok_or(PluginHostError::InvalidScratchWindow { ptr, cap })?;
    if end > mem_size {
        return Err(PluginHostError::InvalidScratchWindow { ptr, cap });
    }
    Ok((uptr, ucap))
}

fn map_wasmi_err(err: wasmi::Error) -> PluginHostError {
    if err.as_trap_code() == Some(TrapCode::OutOfFuel) {
        return PluginHostError::OutOfFuel;
    }
    PluginHostError::GuestTrap(err.to_string())
}

fn accepts_planar(space: &PluginSpaceWire, frame: DesignFrame) -> bool {
    let PluginSpaceWire::Planar2D {
        frame_min,
        frame_max,
        aspect_min,
        aspect_max,
        ..
    } = *space
    else {
        return false;
    };
    if !frame.width.is_finite()
        || !frame.height.is_finite()
        || frame.width <= 0.0
        || frame.height <= 0.0
    {
        return false;
    }
    let aspect = frame.width / frame.height;
    frame.width >= frame_min.width
        && frame.width <= frame_max.width
        && frame.height >= frame_min.height
        && frame.height <= frame_max.height
        && aspect >= aspect_min - 1.0e-4
        && aspect <= aspect_max + 1.0e-4
}

fn accepts_volume(min: Extent3D, max: Extent3D, ext: Extent3D) -> bool {
    ext.is_valid()
        && ext.width_m >= min.width_m - 1.0e-5
        && ext.width_m <= max.width_m + 1.0e-5
        && ext.height_m >= min.height_m - 1.0e-5
        && ext.height_m <= max.height_m + 1.0e-5
        && ext.depth_m >= min.depth_m - 1.0e-5
        && ext.depth_m <= max.depth_m + 1.0e-5
}
