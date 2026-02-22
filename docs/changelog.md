# Changelog

## Unreleased

### Added
- Native `AC1021` (`R2007`) read path in the high-level API (`ezdwg.read`) without compatibility conversion.
- Native `AC1024` (`R2010`) read path in the high-level API (`ezdwg.read`) for `LINE`, `ARC`, and `LWPOLYLINE`.
- Native `AC1027` (`R2013`) read path in the high-level API (`ezdwg.read`) for `LINE`, `ARC`, and `LWPOLYLINE`.
- AC1021 regression suite covering:
  - Rust object/entity decode checks for `LINE`, `ARC`, `LWPOLYLINE`.
  - Python high-level and raw API checks with paired sample files.
  - CLI `inspect` verification for native `decode_version: AC1021`.
- AC1024 regression suite covering high-level and raw geometry checks against paired DXF samples for:
  - `LINE`
  - `ARC`
  - `LWPOLYLINE`
- AC1027 regression suite covering high-level and raw geometry checks against paired DXF samples for:
  - `LINE`
  - `ARC`
  - `LWPOLYLINE`
- R2007+/R2010+/R2013+ regression coverage for:
  - `POINT`
  - `CIRCLE`
  - `ELLIPSE`
- TEXT/MTEXT regression coverage for `R2000`/`R2004` sample pairs.

### Changed
- Removed the external DWG compatibility-conversion path from `ezdwg.read`; AC10xx versions in scope now use native decode paths.
- R2007/R2010/R2013 entity decoding now uses version-aware common header paths for:
  - `LINE`
  - `ARC`
  - `LWPOLYLINE`
  - `POINT`
  - `CIRCLE`
  - `ELLIPSE`
  - `TEXT`
  - `MTEXT`
  - `DIMENSION` (linear/radius/diameter)
  to account for `material flags`, `shadow flags`, R2010 visual-style bits, and the R2013+ ds-binary-data flag.

### Notes
- This release keeps API signatures stable (`ezdwg.read`, `ezdwg.raw`, entity decode functions).
- ARC angles remain radians in `ezdwg.raw` and degrees in the high-level API.
- AC1021/AC1024/AC1027 style-handle and layer-color resolution for LINE/ARC/LWPOLYLINE is currently best-effort on some files.
