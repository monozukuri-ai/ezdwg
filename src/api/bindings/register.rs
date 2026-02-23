pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(detect_version, module)?)?;
    module.add_function(wrap_pyfunction!(write_ac1015_dwg, module)?)?;
    module.add_function(wrap_pyfunction!(write_ac1015_line_dwg, module)?)?;
    module.add_function(wrap_pyfunction!(list_section_locators, module)?)?;
    module.add_function(wrap_pyfunction!(read_section_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_map_entries, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_headers, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_headers_with_type, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_headers_by_type, module)?)?;
    module.add_function(wrap_pyfunction!(read_object_records_by_type, module)?)?;
    module.add_function(wrap_pyfunction!(read_object_records_by_handle, module)?)?;
    module.add_function(wrap_pyfunction!(decode_object_handle_stream_refs, module)?)?;
    module.add_function(wrap_pyfunction!(decode_acis_candidate_infos, module)?)?;
    module.add_function(wrap_pyfunction!(decode_entity_styles, module)?)?;
    module.add_function(wrap_pyfunction!(decode_layer_colors, module)?)?;
    module.add_function(wrap_pyfunction!(decode_line_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_point_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_3dface_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_arc_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_circle_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_line_arc_circle_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_ellipse_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_spline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_text_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_attrib_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_attdef_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_mtext_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_leader_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_hatch_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_tolerance_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_mline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dimension_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_linear_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_ordinate_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_aligned_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_ang3pt_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_ang2ln_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_diameter_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_radius_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_insert_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_minsert_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_insert_minsert_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_block_header_names, module)?)?;
    module.add_function(wrap_pyfunction!(decode_block_entity_names, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_2d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_2d_entities_interpreted,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_lwpolyline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_3d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_3d_with_vertices, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_mesh_entities, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_mesh_with_vertices,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_pface_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_pface_with_faces, module)?)?;
    module.add_function(wrap_pyfunction!(decode_solid_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_trace_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_shape_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_viewport_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_oleframe_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_ole2frame_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_long_transaction_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_region_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_3dsolid_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_body_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_ray_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_xline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_2d_with_vertices, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_2d_with_vertices_interpolated,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_2d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_3d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_mesh_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_pface_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_pface_face_entities, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_2d_with_vertex_data,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_sequence_members, module)?)?;
    Ok(())
}
