/// VIEWPORT rows are dictionaries (angles in radians); `layout` holds the
/// paper-space LAYOUT settings in millimetres.
fn append_write_viewports(
    doc: &mut writer::WriterDocument,
    viewports: Option<Vec<Bound<'_, PyDict>>>,
) -> PyResult<()> {
    use writer::ir::ViewportEntity;
    for d in viewports.unwrap_or_default() {
        check_write_keys(
            &d,
            &[
                "handle",
                "center",
                "width",
                "height",
                "view_target",
                "view_direction",
                "twist_angle",
                "view_height",
                "lens_length",
                "front_clip_z",
                "back_clip_z",
                "snap_angle",
                "view_center",
                "snap_base",
                "snap_spacing",
                "grid_spacing",
                "circle_zoom",
                "status_flags",
                "style_sheet",
                "render_mode",
                "ucs_at_origin",
                "ucs_per_viewport",
                "ucs_origin",
                "ucs_x_axis",
                "ucs_y_axis",
                "ucs_elevation",
                "ucs_ortho_view_type",
                "frozen_layers",
            ],
        )?;
        let handle = write_value(&d, "handle", 0u64)?;
        let v = ViewportEntity::default();
        doc.modelspace
            .push(writer::WriterEntity::Viewport(ViewportEntity {
                common: writer::CommonEntityProps {
                    handle: (handle != 0).then_some(handle),
                    ..Default::default()
                },
                center: write_value(&d, "center", v.center)?,
                width: write_value(&d, "width", v.width)?,
                height: write_value(&d, "height", v.height)?,
                view_target: write_value(&d, "view_target", v.view_target)?,
                view_direction: write_value(&d, "view_direction", v.view_direction)?,
                twist_angle: write_value(&d, "twist_angle", v.twist_angle)?,
                view_height: write_value(&d, "view_height", v.view_height)?,
                lens_length: write_value(&d, "lens_length", v.lens_length)?,
                front_clip_z: write_value(&d, "front_clip_z", v.front_clip_z)?,
                back_clip_z: write_value(&d, "back_clip_z", v.back_clip_z)?,
                snap_angle: write_value(&d, "snap_angle", v.snap_angle)?,
                view_center: write_value(&d, "view_center", v.view_center)?,
                snap_base: write_value(&d, "snap_base", v.snap_base)?,
                snap_spacing: write_value(&d, "snap_spacing", v.snap_spacing)?,
                grid_spacing: write_value(&d, "grid_spacing", v.grid_spacing)?,
                circle_zoom: write_value(&d, "circle_zoom", v.circle_zoom)?,
                status_flags: write_value(&d, "status_flags", v.status_flags)?,
                style_sheet: write_value(&d, "style_sheet", v.style_sheet)?,
                render_mode: write_value(&d, "render_mode", v.render_mode)?,
                ucs_at_origin: write_value(&d, "ucs_at_origin", v.ucs_at_origin)?,
                ucs_per_viewport: write_value(&d, "ucs_per_viewport", v.ucs_per_viewport)?,
                ucs_origin: write_value(&d, "ucs_origin", v.ucs_origin)?,
                ucs_x_axis: write_value(&d, "ucs_x_axis", v.ucs_x_axis)?,
                ucs_y_axis: write_value(&d, "ucs_y_axis", v.ucs_y_axis)?,
                ucs_elevation: write_value(&d, "ucs_elevation", v.ucs_elevation)?,
                ucs_ortho_view_type: write_value(&d, "ucs_ortho_view_type", v.ucs_ortho_view_type)?,
                frozen_layers: write_value(&d, "frozen_layers", Vec::new())?,
            }));
    }
    Ok(())
}

/// Paper size and plot settings of the paper-space layout (`Layout1`).
fn apply_write_layout(
    doc: &mut writer::WriterDocument,
    layout: Option<Bound<'_, PyDict>>,
) -> PyResult<()> {
    let Some(d) = layout else {
        return Ok(());
    };
    check_write_keys(
        &d,
        &[
            "name",
            "paper_width",
            "paper_height",
            "margins",
            "paper_size",
            "plot_origin",
            "paper_units",
            "plot_rotation",
            "plot_type",
            "plot_layout_flags",
            "plot_window",
            "scale_type",
            "scale_numerator",
            "scale_denominator",
            "paper_image_origin",
            "page_setup_name",
            "printer",
            "plot_view_name",
            "style_sheet",
        ],
    )?;
    let l = &mut doc.layout;
    l.name = write_value(&d, "name", l.name.clone())?;
    l.paper_width = write_value(&d, "paper_width", l.paper_width)?;
    l.paper_height = write_value(&d, "paper_height", l.paper_height)?;
    l.margins = write_value(&d, "margins", l.margins)?;
    l.paper_size = write_value(&d, "paper_size", l.paper_size.clone())?;
    l.plot_origin = write_value(&d, "plot_origin", l.plot_origin)?;
    l.paper_units = write_value(&d, "paper_units", l.paper_units)?;
    l.plot_rotation = write_value(&d, "plot_rotation", l.plot_rotation)?;
    l.plot_type = write_value(&d, "plot_type", l.plot_type)?;
    l.plot_layout_flags = write_value(&d, "plot_layout_flags", l.plot_layout_flags)?;
    l.plot_window = write_value(&d, "plot_window", l.plot_window)?;
    l.scale_type = write_value(&d, "scale_type", l.scale_type)?;
    l.scale_numerator = write_value(&d, "scale_numerator", l.scale_numerator)?;
    l.scale_denominator = write_value(&d, "scale_denominator", l.scale_denominator)?;
    l.paper_image_origin = write_value(&d, "paper_image_origin", l.paper_image_origin)?;
    l.page_setup_name = write_value(&d, "page_setup_name", l.page_setup_name.clone())?;
    l.printer = write_value(&d, "printer", l.printer.clone())?;
    l.plot_view_name = write_value(&d, "plot_view_name", l.plot_view_name.clone())?;
    l.style_sheet = write_value(&d, "style_sheet", l.style_sheet.clone())?;
    Ok(())
}
