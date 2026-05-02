//! Round-trip and basic-construction tests.

use gifti_rs::model::CoordSystem;
use gifti_rs::{
    intent, parse_str, serialize, ArrayData, ArrayIndexOrder, DataArray, DataType, Encoding,
    Endian, GiftiImage,
};

fn make_minimal_surface() -> GiftiImage {
    let pointset = DataArray {
        intent: intent::POINTSET,
        datatype: DataType::Float32 as i32,
        array_index_order: ArrayIndexOrder::RowMajor,
        dims: vec![3, 3],
        encoding: Encoding::Base64Binary,
        endian: Endian::Little,
        ext_filename: None,
        ext_offset: None,
        coordsys: vec![CoordSystem {
            data_space: "NIFTI_XFORM_UNKNOWN".to_string(),
            transformed_space: "NIFTI_XFORM_UNKNOWN".to_string(),
            xform: CoordSystem::identity(),
        }],
        meta: vec![
            ("VolGeomC_R".to_string(), "0.500000".to_string()),
            ("VolGeomC_A".to_string(), "-1.250000".to_string()),
            ("VolGeomC_S".to_string(), "2.000000".to_string()),
        ],
        data: ArrayData::Float32(vec![
            0.0, 0.0, 0.0, // v0
            1.0, 0.0, 0.0, // v1
            0.0, 1.0, 0.0, // v2
        ]),
    };

    let triangle = DataArray {
        intent: intent::TRIANGLE,
        datatype: DataType::Int32 as i32,
        array_index_order: ArrayIndexOrder::RowMajor,
        dims: vec![1, 3],
        encoding: Encoding::GZipBase64Binary,
        endian: Endian::Little,
        ext_filename: None,
        ext_offset: None,
        coordsys: vec![],
        meta: vec![],
        data: ArrayData::Int32(vec![0, 1, 2]),
    };

    GiftiImage {
        version: "1.0".to_string(),
        num_data_arrays: 2,
        meta: vec![("UserName".to_string(), "tester".to_string())],
        label_table: None,
        data_arrays: vec![pointset, triangle],
    }
}

#[test]
fn round_trip_in_memory() {
    let original = make_minimal_surface();
    let xml = serialize(&original).expect("serialize");
    let parsed = parse_str(&xml).expect("parse");

    assert_eq!(parsed.version, original.version);
    assert_eq!(parsed.data_arrays.len(), original.data_arrays.len());

    for (a, b) in parsed.data_arrays.iter().zip(original.data_arrays.iter()) {
        assert_eq!(a.intent, b.intent);
        assert_eq!(a.datatype, b.datatype);
        assert_eq!(a.dims, b.dims);
        assert_eq!(a.encoding, b.encoding);
        assert_eq!(a.array_index_order, b.array_index_order);
        assert_eq!(a.meta, b.meta);
        assert_eq!(a.coordsys.len(), b.coordsys.len());
        for (cs_a, cs_b) in a.coordsys.iter().zip(b.coordsys.iter()) {
            assert_eq!(cs_a.data_space, cs_b.data_space);
            assert_eq!(cs_a.transformed_space, cs_b.transformed_space);
            assert_eq!(cs_a.xform, cs_b.xform);
        }
    }

    // POINTSET data should be byte-equal because we wrote f32 little-endian.
    let (orig_da, parsed_da) = (&original.data_arrays[0], &parsed.data_arrays[0]);
    if let (ArrayData::Float32(o), ArrayData::Float32(p)) = (&orig_da.data, &parsed_da.data) {
        assert_eq!(o, p);
    } else {
        panic!("expected Float32 POINTSET");
    }
    if let (ArrayData::Int32(o), ArrayData::Int32(p)) =
        (&original.data_arrays[1].data, &parsed.data_arrays[1].data)
    {
        assert_eq!(o, p);
    } else {
        panic!("expected Int32 TRIANGLE");
    }
}

#[test]
fn cras_helpers_detect_and_zero() {
    let mut img = make_minimal_surface();
    let pointset = gifti_rs::cras::pointset_array(&img).expect("has POINTSET");
    let cras = gifti_rs::cras::read_cras(pointset);
    assert!((cras[0] - 0.5).abs() < 1e-9);
    assert!((cras[1] - -1.25).abs() < 1e-9);
    assert!((cras[2] - 2.0).abs() < 1e-9);
    assert!(gifti_rs::cras::has_cras(pointset));

    let pointset_mut = gifti_rs::cras::pointset_array_mut(&mut img).unwrap();
    gifti_rs::cras::zero_cras_meta(pointset_mut);
    assert!(!gifti_rs::cras::has_cras(pointset_mut));

    // Survives round-trip with zeroed values still present in metadata.
    let xml = serialize(&img).unwrap();
    let reparsed = parse_str(&xml).unwrap();
    let pointset = gifti_rs::cras::pointset_array(&reparsed).unwrap();
    assert_eq!(
        gifti_rs::model::meta_get(&pointset.meta, "VolGeomC_R"),
        Some("0.000000")
    );
}

#[test]
fn ascii_encoding_round_trips() {
    let mut img = make_minimal_surface();
    img.data_arrays[0].encoding = Encoding::Ascii;
    img.data_arrays[1].encoding = Encoding::Ascii;
    let xml = serialize(&img).unwrap();
    let parsed = parse_str(&xml).unwrap();
    if let (ArrayData::Float32(o), ArrayData::Float32(p)) =
        (&img.data_arrays[0].data, &parsed.data_arrays[0].data)
    {
        assert_eq!(o, p);
    } else {
        panic!("expected Float32");
    }
}
