fn append_write_blocks(
    doc: &mut writer::WriterDocument,
    blocks: Option<Vec<Bound<'_, PyDict>>>,
    inserts: Option<Vec<Bound<'_, PyDict>>>,
    attdefs: Option<Vec<Bound<'_, PyDict>>>,
    attribs: Option<Vec<Bound<'_, PyDict>>>,
) -> PyResult<()> {
    use writer::ir::{AttributeEntity, BlockDef, InsertEntity};
    for d in blocks.unwrap_or_default() {
        check_write_keys(&d, &["name", "base_point", "description"])?;
        doc.blocks.push(BlockDef {
            name: write_value(&d, "name", String::new())?,
            base_point: write_value(&d, "base_point", (0., 0., 0.))?,
            description: write_value(&d, "description", String::new())?,
            entities: Vec::new(),
        });
    }
    for d in inserts.unwrap_or_default() {
        check_write_keys(&d, &["handle", "name", "insert", "scale", "rotation"])?;
        let handle = write_value(&d, "handle", 0u64)?;
        doc.modelspace
            .push(writer::WriterEntity::Insert(InsertEntity {
                common: writer::CommonEntityProps {
                    handle: (handle != 0).then_some(handle),
                    ..Default::default()
                },
                block_name: write_value(&d, "name", String::new())?,
                insert: write_value(&d, "insert", (0., 0., 0.))?,
                scale: write_value(&d, "scale", (1., 1., 1.))?,
                rotation_rad: write_value(&d, "rotation", 0.)?,
                attributes: Vec::new(),
            }));
    }
    for (definition, rows) in [(true, attdefs), (false, attribs)] {
        for d in rows.unwrap_or_default() {
            check_write_keys(
                &d,
                &[
                    "handle",
                    "text",
                    "tag",
                    "prompt",
                    "insert",
                    "height",
                    "rotation",
                    "attribute_flags",
                    "field_length",
                ],
            )?;
            let handle = write_value(&d, "handle", 0u64)?;
            let attr = AttributeEntity {
                text: writer::TextEntity {
                    common: writer::CommonEntityProps {
                        handle: (handle != 0).then_some(handle),
                        ..Default::default()
                    },
                    text: write_value(&d, "text", String::new())?,
                    insert: write_value(&d, "insert", (0., 0., 0.))?,
                    height: write_value(&d, "height", 1.)?,
                    rotation_rad: write_value(&d, "rotation", 0.)?,
                },
                tag: write_value(&d, "tag", String::new())?,
                prompt: write_value(&d, "prompt", String::new())?,
                flags: write_value(&d, "attribute_flags", 0)?,
                field_length: write_value(&d, "field_length", 0)?,
            };
            doc.modelspace.push(if definition {
                writer::WriterEntity::Attdef(attr)
            } else {
                writer::WriterEntity::Attrib(attr)
            });
        }
    }
    Ok(())
}

/// Owner values are block names for definition contents, or INSERT handles for ATTRIB.
fn assign_write_owners(
    doc: &mut writer::WriterDocument,
    owners: Option<HashMap<u64, Bound<'_, PyAny>>>,
) -> PyResult<()> {
    let mut owners = owners.unwrap_or_default();
    let mut attributes: HashMap<u64, Vec<writer::ir::AttributeEntity>> = HashMap::new();
    let mut entities = Vec::new();
    for e in std::mem::take(&mut doc.modelspace) {
        if let writer::WriterEntity::Attrib(attr) = e {
            let owner = attr
                .text
                .common
                .handle
                .and_then(|h| owners.remove(&h))
                .ok_or_else(|| PyValueError::new_err("ATTRIB requires an INSERT owner"))?;
            let handle = owner
                .extract::<u64>()
                .map_err(|_| PyValueError::new_err("ATTRIB owner must be an INSERT handle"))?;
            attributes.entry(handle).or_default().push(attr);
        } else {
            entities.push(e);
        }
    }
    for mut e in entities {
        if let writer::WriterEntity::Insert(insert) = &mut e {
            if let Some(h) = insert.common.handle {
                insert.attributes = attributes.remove(&h).unwrap_or_default();
            }
        }
        if let Some(owner) = e.common().handle.and_then(|h| owners.remove(&h)) {
            let name = owner
                .extract::<String>()
                .map_err(|_| PyValueError::new_err("entity owner must be a block name"))?;
            let block = doc
                .blocks
                .iter_mut()
                .find(|b| b.name.to_uppercase() == name.to_uppercase())
                .ok_or_else(|| PyValueError::new_err(format!("undefined owner block: {name}")))?;
            block.entities.push(e);
        } else {
            doc.modelspace.push(e);
        }
    }
    if !attributes.is_empty() || !owners.is_empty() {
        return Err(PyValueError::new_err("unknown entity/INSERT owner handle"));
    }
    Ok(())
}
