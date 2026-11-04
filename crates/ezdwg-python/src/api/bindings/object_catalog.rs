/// Header results and type-specific offsets of one immutable decoded file.
///
/// Index positions, rather than handles, preserve duplicate handles/offsets and
/// the original object-map order. Failed headers are retained too: a strict
/// typed read must still fail when it reaches an unrelated malformed record,
/// but a limit reached before that record must keep succeeding.
#[derive(Default)]
struct ObjectCatalog {
    headers: HashMap<u32, crate::core::result::Result<ApiObjectHeader>>,
    by_type: HashMap<u16, Vec<usize>>,
    failures: Vec<usize>,
}

impl ObjectCatalog {
    fn build(
        index: &objects::ObjectIndex,
        mut read_header: impl FnMut(u32) -> crate::core::result::Result<ApiObjectHeader>,
    ) -> Self {
        let mut catalog = Self {
            headers: HashMap::with_capacity(index.len()),
            ..Self::default()
        };
        for (position, object) in index.objects.iter().enumerate() {
            let header = catalog
                .headers
                .entry(object.offset)
                .or_insert_with(|| read_header(object.offset));
            match header {
                Ok(header) => catalog
                    .by_type
                    .entry(header.type_code)
                    .or_default()
                    .push(position),
                Err(_) => catalog.failures.push(position),
            }
        }
        catalog
    }

    fn type_names(&self, dynamic_types: &HashMap<u16, String>) -> Vec<String> {
        let mut types: Vec<_> = self
            .by_type
            .iter()
            .filter_map(|(&code, positions)| {
                positions
                    .first()
                    .map(|&first| (first, resolved_type_name(code, dynamic_types)))
            })
            .collect();
        types.sort_unstable_by_key(|(first, _)| *first);
        let mut seen = HashSet::new();
        types
            .into_iter()
            .filter_map(|(_, name)| seen.insert(name.clone()).then_some(name))
            .collect()
    }

    fn matching_objects(
        &self,
        index: &objects::ObjectIndex,
        best_effort: bool,
        matches: impl Fn(u16) -> bool,
    ) -> Vec<objects::ObjectRef> {
        let mut positions = if best_effort {
            Vec::new()
        } else {
            self.failures.clone()
        };
        let mut runs = usize::from(!positions.is_empty());
        for (&type_code, offsets) in &self.by_type {
            if matches(type_code) {
                positions.extend_from_slice(offsets);
                runs += 1;
            }
        }
        if runs > 1 {
            positions.sort_unstable();
        }
        positions.into_iter().map(|i| index.objects[i]).collect()
    }
}

/// Binding-local decoder over the bytes owned by FileDecodeState. Standalone
/// callers of build_decoder keep working without a catalog; decoders obtained
/// through decoded_file share the lazily built catalog with every other pass.
struct FileDecoder<'a> {
    inner: decoder::Decoder<'a>,
    catalog: Option<(&'a objects::ObjectIndex, &'a OnceLock<ObjectCatalog>)>,
}

impl<'a> std::ops::Deref for FileDecoder<'a> {
    type Target = decoder::Decoder<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> FileDecoder<'a> {
    fn with_objects_section(mut self, section: Arc<Vec<u8>>) -> Self {
        self.inner = self.inner.with_objects_section(section);
        self
    }

    fn with_object_catalog(
        mut self,
        index: &'a objects::ObjectIndex,
        catalog: &'a OnceLock<ObjectCatalog>,
    ) -> Self {
        self.catalog = Some((index, catalog));
        self
    }

    fn object_catalog(&self) -> Option<&ObjectCatalog> {
        let (index, slot) = self.catalog?;
        Some(slot.get_or_init(|| {
            // Only headers are retained. Borrow record bytes from the already
            // decompressed section instead of copying body/raw for every entry
            // while building the catalog. Entity readers keep their existing
            // record ownership and version dispatch.
            let section = self.inner.objects_section();
            ObjectCatalog::build(index, |offset| {
                let record = match section.as_ref() {
                    Some(data) if is_r2010_plus_version(self.version()) => {
                        objects::object_record::parse_object_record_r2010(data, offset)?
                    }
                    Some(data) => objects::object_record::parse_object_record(data, offset)?,
                    None => self.inner.parse_object_record(offset)?,
                };
                parse_object_header_for_version(&record, self.version())
            })
        }))
    }

    fn object_header(&self, offset: u32) -> crate::core::result::Result<ApiObjectHeader> {
        if let Some(header) = self.object_catalog().and_then(|c| c.headers.get(&offset)) {
            return header.clone();
        }
        let record = self.inner.parse_object_record(offset)?;
        parse_object_header_for_version(&record, self.version())
    }

    fn record_header(
        &self,
        record: &objects::ObjectRecord<'_>,
    ) -> crate::core::result::Result<ApiObjectHeader> {
        // A targeted read by handle need not build the full catalog. Reuse it
        // when a typed/header pass has already initialized it.
        if let Some(header) = self
            .catalog
            .and_then(|(_, slot)| slot.get())
            .and_then(|c| c.headers.get(&record.offset))
        {
            return header.clone();
        }
        parse_object_header_for_version(record, self.version())
    }

    fn matching_objects(
        &self,
        index: &objects::ObjectIndex,
        best_effort: bool,
        matches: impl Fn(u16) -> bool,
    ) -> Vec<objects::ObjectRef> {
        match self.object_catalog() {
            Some(catalog) => catalog.matching_objects(index, best_effort, matches),
            // Existing type checks in the callers also cover standalone decoders.
            None => index.objects.clone(),
        }
    }
}

/// Unique resolved DWG type names, in first object-map occurrence order.
/// Includes non-entity and unsupported types, just like the header-list API.
#[pyfunction]
fn list_object_type_names(path: &str) -> PyResult<Vec<String>> {
    let shared = decoded_file(path)?;
    let decoder = shared.decoder()?;
    let catalog = decoder
        .object_catalog()
        .ok_or_else(|| PyValueError::new_err("decoded file has no object catalog"))?;
    if !shared.best_effort {
        if let Some(&position) = catalog.failures.first() {
            decoder
                .object_header(shared.index.objects[position].offset)
                .map_err(to_py_err)?;
        }
    }
    Ok(catalog.type_names(&shared.dynamic_types))
}

/// Handles of an exact resolved DWG type name, ignoring surrounding whitespace
/// and case. Keep object-map order and duplicate handles, like the header API.
#[pyfunction(signature = (path, type_name, limit=None))]
fn list_object_handles_by_type_name(
    path: &str,
    type_name: &str,
    limit: Option<usize>,
) -> PyResult<Vec<u64>> {
    let target = type_name.trim().to_uppercase();
    if target.is_empty() || limit == Some(0) {
        return Ok(Vec::new());
    }
    let shared = decoded_file(path)?;
    let decoder = shared.decoder()?;
    let matches = |code| {
        resolved_type_name(code, &shared.dynamic_types)
            .trim()
            .to_uppercase()
            == target
    };
    let mut handles = Vec::new();
    for object in decoder.matching_objects(&shared.index, shared.best_effort, matches) {
        let Some(header) = object_header_at(&decoder, object.offset, shared.best_effort)? else {
            continue;
        };
        if !matches(header.type_code) {
            continue;
        }
        handles.push(object.handle.0);
        if limit.is_some_and(|limit| handles.len() >= limit) {
            break;
        }
    }
    Ok(handles)
}

#[cfg(test)]
mod object_catalog_tests {
    use super::*;
    use std::cell::Cell;

    fn header(type_code: u16) -> ApiObjectHeader {
        ApiObjectHeader {
            data_size: 12,
            type_code,
            handle_stream_size_bits: Some(5),
            data_start_bit: Some(8),
        }
    }

    #[test]
    fn compact_type_names_keep_first_occurrence_and_merge_dynamic_aliases() {
        let index = objects::ObjectIndex::from_objects(
            (0..5)
                .map(|offset| objects::ObjectRef {
                    handle: objects::Handle(u64::from(offset)),
                    offset,
                })
                .collect(),
        );
        let catalog = ObjectCatalog::build(&index, |offset| {
            if offset == 1 {
                Err(DwgError::new(ErrorKind::Format, "broken header"))
            } else {
                Ok(header([501, 0, 0x12, 0x13, 502][offset as usize]))
            }
        });
        let dynamic = HashMap::from([
            (501, "LINE".to_string()),
            (502, "CUSTOM_ENTITY".to_string()),
        ]);
        assert_eq!(
            catalog.type_names(&dynamic),
            ["LINE", "CIRCLE", "CUSTOM_ENTITY"]
        );
        let lines = catalog.matching_objects(&index, true, |code| {
            resolved_type_name(code, &dynamic).trim().to_uppercase() == "LINE"
        });
        assert_eq!(lines.iter().map(|o| o.handle.0).collect::<Vec<_>>(), [0, 3]);
    }

    #[test]
    fn decoders_of_one_file_share_the_same_catalog() {
        for sample in [
            "line_R14.dwg",
            "line_2000.dwg",
            "line_2004.dwg",
            "line_2007.dwg",
            "line_2010.dwg",
            "line_2013.dwg",
            "acadsharp/sample_AC1032.dwg",
        ] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../test_dwg")
                .join(sample);
            let bytes = std::fs::read(path).unwrap();
            let index = build_decoder(&bytes).unwrap().build_object_index().unwrap();
            let slot = OnceLock::new();
            let first = build_decoder(&bytes)
                .unwrap()
                .with_object_catalog(&index, &slot);
            // This is how DecodedFile preloads modern object sections.
            first.parse_object_record(index.objects[0].offset).unwrap();
            let second = build_decoder(&bytes)
                .unwrap()
                .with_object_catalog(&index, &slot);
            assert!(slot.get().is_none());
            let first_catalog = first.object_catalog().unwrap();
            assert!(std::ptr::eq(
                first_catalog,
                second.object_catalog().unwrap()
            ));
            for object in &index.objects {
                let record = first.parse_object_record(object.offset).unwrap();
                let expected = parse_object_header_for_version(&record, first.version()).unwrap();
                let actual = second.object_header(object.offset).unwrap();
                assert_eq!(actual.type_code, expected.type_code, "{sample}");
                assert_eq!(actual.data_size, expected.data_size, "{sample}");
                assert_eq!(actual.data_start_bit, expected.data_start_bit, "{sample}");
                assert_eq!(
                    actual.handle_stream_size_bits, expected.handle_stream_size_bits,
                    "{sample}"
                );
            }
        }
    }

    #[test]
    fn catalog_decodes_each_offset_once_and_preserves_map_order_and_duplicates() {
        let index = objects::ObjectIndex::from_objects(
            [(9, 30), (2, 10), (9, 20), (3, 30)]
                .map(|(handle, offset)| objects::ObjectRef {
                    handle: objects::Handle(handle),
                    offset,
                })
                .to_vec(),
        );
        let calls = Cell::new(0);
        let catalog = ObjectCatalog::build(&index, |offset| {
            calls.set(calls.get() + 1);
            Ok(header(if offset == 10 { 0x12 } else { 0x13 }))
        });
        assert_eq!(calls.get(), 3);
        let lines = catalog.matching_objects(&index, false, |t| t == 0x13);
        assert_eq!(
            lines
                .iter()
                .map(|o| (o.handle.0, o.offset))
                .collect::<Vec<_>>(),
            [(9, 30), (9, 20), (3, 30)]
        );
        let all = catalog.matching_objects(&index, false, |_| true);
        assert_eq!(
            all.iter().map(|o| o.offset).collect::<Vec<_>>(),
            [30, 10, 20, 30]
        );
        assert_eq!(
            catalog.headers[&30]
                .as_ref()
                .unwrap()
                .handle_stream_size_bits,
            Some(5)
        );
    }

    #[test]
    fn strict_reads_keep_unrelated_errors_between_matching_records() {
        let index = objects::ObjectIndex::from_objects(
            (0..4)
                .map(|offset| objects::ObjectRef {
                    handle: objects::Handle(u64::from(offset)),
                    offset,
                })
                .collect(),
        );
        let catalog = ObjectCatalog::build(&index, |offset| {
            if offset == 1 {
                Err(DwgError::new(ErrorKind::Format, "broken header").with_offset(1))
            } else {
                Ok(header(if offset == 2 { 0x12 } else { 0x13 }))
            }
        });
        let strict = catalog.matching_objects(&index, false, |t| t == 0x13);
        assert_eq!(
            strict.iter().map(|o| o.offset).collect::<Vec<_>>(),
            [0, 1, 3]
        );
        // Reaching a limit of one returns before the error at position one.
        assert_eq!(
            strict.iter().take(1).map(|o| o.offset).collect::<Vec<_>>(),
            [0]
        );
        let best_effort = catalog.matching_objects(&index, true, |t| t == 0x13);
        assert_eq!(
            best_effort.iter().map(|o| o.offset).collect::<Vec<_>>(),
            [0, 3]
        );
        assert!(catalog.headers[&1].is_err());
    }

    #[test]
    fn dynamic_class_aliases_share_the_builtin_type_selection() {
        let index = objects::ObjectIndex::from_objects(
            (0..3)
                .map(|offset| objects::ObjectRef {
                    handle: objects::Handle(u64::from(offset)),
                    offset,
                })
                .collect(),
        );
        let catalog = ObjectCatalog::build(&index, |offset| {
            Ok(header([501, 0x12, 0x13][offset as usize]))
        });
        let dynamic_types = HashMap::from([(501, "LINE".to_string())]);
        let lines = catalog.matching_objects(&index, true, |t| {
            matches_type_name(t, 0x13, "LINE", &dynamic_types)
        });
        assert_eq!(lines.iter().map(|o| o.offset).collect::<Vec<_>>(), [0, 2]);
    }
}
