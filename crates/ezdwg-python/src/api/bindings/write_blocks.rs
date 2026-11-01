/// Block definitions (`name`, `base_point`, `description`); their contents arrive
/// as entities whose `entity_owners` entry names the block.
fn append_block_defs(
    doc: &mut writer::WriterDocument,
    blocks: Option<Vec<Bound<'_, PyDict>>>,
) -> PyResult<()> {
    use writer::ir::BlockDef;
    for d in blocks.unwrap_or_default() {
        check_write_keys(&d, &["name", "base_point", "description"])?;
        doc.blocks.push(BlockDef {
            name: write_value(&d, "name", String::new())?,
            base_point: write_value(&d, "base_point", (0., 0., 0.))?,
            description: write_value(&d, "description", String::new())?,
            entities: Vec::new(),
        });
    }
    Ok(())
}

fn build_write_inserts(
    out: &mut Vec<writer::WriterEntity>,
    inserts: Option<Vec<Bound<'_, PyDict>>>,
) -> PyResult<()> {
    use writer::ir::InsertEntity;
    for d in inserts.unwrap_or_default() {
        check_write_keys(&d, &["handle", "name", "insert", "scale", "rotation"])?;
        let handle = write_value(&d, "handle", 0u64)?;
        out.push(writer::WriterEntity::Insert(Box::new(InsertEntity {
            common: writer::CommonEntityProps {
                handle: (handle != 0).then_some(handle),
                ..Default::default()
            },
            block_name: write_value(&d, "name", String::new())?,
            insert: write_value(&d, "insert", (0., 0., 0.))?,
            scale: write_value(&d, "scale", (1., 1., 1.))?,
            rotation_rad: write_value(&d, "rotation", 0.)?,
            attributes: Vec::new(),
        })));
    }
    Ok(())
}

/// ATTDEF (`definition`) or ATTRIB rows.
fn build_write_attributes(
    out: &mut Vec<writer::WriterEntity>,
    rows: Option<Vec<Bound<'_, PyDict>>>,
    definition: bool,
) -> PyResult<()> {
    use writer::ir::AttributeEntity;
    {
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
            out.push(if definition {
                writer::WriterEntity::Attdef(Box::new(attr))
            } else {
                writer::WriterEntity::Attrib(Box::new(attr))
            });
        }
    }
    Ok(())
}
