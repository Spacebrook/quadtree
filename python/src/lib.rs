use quadtree::quadtree::{Config, QuadTree, RelocationRequest};
use quadtree::shapes::{Circle, Rectangle, Shape, ShapeEnum};

use pyo3::exceptions::PyTypeError;
#[cfg(feature = "pyo3")]
use pyo3::prelude::*;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::pymodule;
use pyo3::types::PyTuple;
use pyo3::types::{PyList, PyModule};
use pyo3::IntoPy;
use pyo3::Py;
use pyo3::PyObject;
use pyo3::PyResult;
use pyo3::Python;

#[derive(Debug, Clone)]
#[pyclass(name = "Circle")]
struct PyCircle {
    x: f32,
    y: f32,
    radius: f32,
}

#[pymethods]
impl PyCircle {
    #[new]
    pub fn new(x: f32, y: f32, radius: f32) -> Self {
        PyCircle { x, y, radius }
    }
}

#[derive(Debug, Clone)]
#[pyclass(name = "Rectangle")]
struct PyRectangle {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[pymethods]
impl PyRectangle {
    #[new]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        PyRectangle {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Clone)]
#[pyclass(name = "Config")]
pub struct PyConfig {
    pool_size: usize,
    node_capacity: usize,
    max_depth: usize,
}

#[pymethods]
impl PyConfig {
    #[new]
    pub fn new(pool_size: usize, node_capacity: usize, max_depth: usize) -> Self {
        PyConfig {
            pool_size,
            node_capacity,
            max_depth,
        }
    }
}

#[pymodule]
fn pyquadtree(_py: Python, m: &PyModule) -> PyResult<()> {
    #[pyclass(name = "QuadTree", unsendable)]
    struct QuadTreeWrapper {
        quadtree: QuadTree,
    }

    #[pymethods]
    impl QuadTreeWrapper {
        #[new]
        pub fn new(bounding_box: PyRectangle) -> Self {
            let bounding_rect = Rectangle {
                x: bounding_box.x,
                y: bounding_box.y,
                width: bounding_box.width,
                height: bounding_box.height,
            };
            QuadTreeWrapper {
                quadtree: QuadTree::new(bounding_rect),
            }
        }

        #[staticmethod]
        pub fn new_with_config(bounding_box: PyRectangle, config: PyConfig) -> Self {
            let bounding_rect = Rectangle {
                x: bounding_box.x,
                y: bounding_box.y,
                width: bounding_box.width,
                height: bounding_box.height,
            };
            let rust_config = Config {
                pool_size: config.pool_size,
                node_capacity: config.node_capacity,
                max_depth: config.max_depth,
            };
            QuadTreeWrapper {
                quadtree: QuadTree::new_with_config(bounding_rect, rust_config),
            }
        }

        pub fn insert(
            &mut self,
            py: Python,
            value: u32,
            shape: PyObject,
            entity_type: Option<u32>,
        ) -> PyResult<()> {
            let shape = self.extract_shape(py, shape)?;
            self.quadtree.insert(value, shape, entity_type);
            Ok(())
        }

        pub fn delete(&mut self, value: u32) {
            self.quadtree.delete(value);
        }

        pub fn collisions(&self, py: Python, shape: PyObject) -> PyResult<Vec<u32>> {
            return self.collisions_filter(py, shape, None);
        }

        pub fn collisions_filter(
            &self,
            py: Python,
            shape: PyObject,
            entity_types: Option<&PyList>,
        ) -> PyResult<Vec<u32>> {
            let shape = self.extract_shape(py, shape)?;

            let entity_types = self.extract_entity_types(entity_types)?;

            let mut collisions = Vec::new();
            self.quadtree
                .collisions_filter(shape, entity_types, &mut collisions);
            Ok(collisions)
        }

        pub fn collisions_batch(&self, py: Python, shapes: &PyList) -> PyResult<Vec<Vec<u32>>> {
            self.collisions_batch_filter(py, shapes, None)
        }

        pub fn collisions_batch_filter(
            &self,
            py: Python,
            shapes: &PyList,
            entity_types: Option<&PyList>,
        ) -> PyResult<Vec<Vec<u32>>> {
            let shapes: Vec<ShapeEnum> = shapes
                .iter()
                .map(|shape| self.extract_shape(py, shape.into()))
                .collect::<Result<_, _>>()?;

            let entity_types = self.extract_entity_types(entity_types)?;

            Ok(self.quadtree.collisions_batch_filter(shapes, entity_types))
        }

        pub fn relocate(
            &mut self,
            py: Python,
            value: u32,
            shape: PyObject,
            entity_type: Option<u32>,
        ) -> PyResult<()> {
            let shape = self.extract_shape(py, shape)?;
            self.quadtree.relocate(value, shape, entity_type);
            Ok(())
        }

        pub fn relocate_batch(
            &mut self,
            py: Python,
            relocation_requests: Vec<&PyTuple>,
        ) -> PyResult<()> {
            // Convert the Python tuples into Rust RelocationRequest objects
            let requests: Vec<RelocationRequest> = relocation_requests
                .into_iter()
                .map(|tuple| {
                    let value = tuple.get_item(0).unwrap().extract::<u32>().unwrap();
                    let shape = self
                        .extract_shape(py, tuple.get_item(1).unwrap().into())
                        .unwrap();
                    let entity_type: Option<u32> = match tuple.get_item(2).unwrap() {
                        obj if obj.is_none() => None, // Check if it's a Python None
                        obj => Some(obj.extract::<u32>().unwrap()),
                    };
                    RelocationRequest {
                        value,
                        shape,
                        entity_type,
                    }
                })
                .collect();

            self.quadtree.relocate_batch(requests);

            Ok(())
        }

        pub fn all_node_bounding_boxes(&self) -> Vec<(f32, f32, f32, f32)> {
            let mut bounding_boxes = Vec::new();
            self.quadtree.all_node_bounding_boxes(&mut bounding_boxes);
            bounding_boxes
                .into_iter()
                .map(|rect| (rect.x, rect.y, rect.width, rect.height))
                .collect()
        }

        pub fn all_shapes(&self, py: Python) -> PyResult<Vec<PyObject>> {
            let mut shapes = Vec::new();
            self.quadtree.all_shapes(&mut shapes);
            let mut py_shapes = Vec::new();
            for shape in shapes {
                let py_shape = if let Some(circle) = shape.as_any().downcast_ref::<Circle>() {
                    Py::new(
                        py,
                        PyCircle {
                            x: circle.x,
                            y: circle.y,
                            radius: circle.radius,
                        },
                    )?
                    .into_py(py)
                } else if let Some(rect) = shape.as_any().downcast_ref::<Rectangle>() {
                    Py::new(
                        py,
                        PyRectangle {
                            x: rect.x,
                            y: rect.y,
                            width: rect.width,
                            height: rect.height,
                        },
                    )?
                    .into_py(py)
                } else {
                    return Err(PyTypeError::new_err("Unknown shape"));
                };
                py_shapes.push(py_shape);
            }
            Ok(py_shapes)
        }
    }

    impl QuadTreeWrapper {
        fn extract_shape(&self, py: Python, shape: PyObject) -> PyResult<ShapeEnum> {
            if let Ok(py_rectangle) = shape.extract::<PyRectangle>(py) {
                Ok(ShapeEnum::Rectangle(Rectangle {
                    x: py_rectangle.x,
                    y: py_rectangle.y,
                    width: py_rectangle.width,
                    height: py_rectangle.height,
                }))
            } else if let Ok(py_circle) = shape.extract::<PyCircle>(py) {
                Ok(ShapeEnum::Circle(Circle::new(
                    py_circle.x,
                    py_circle.y,
                    py_circle.radius,
                )))
            } else {
                Err(PyTypeError::new_err(
                    "Expected a Rectangle or Circle object",
                ))
            }
        }

        fn extract_entity_types(
            &self,
            entity_types: Option<&PyList>,
        ) -> PyResult<Option<Vec<u32>>> {
            match entity_types {
                Some(entity_types_list) => {
                    let et: Result<Vec<u32>, _> = entity_types_list
                        .iter()
                        .map(|item| item.extract::<u32>())
                        .collect();
                    Ok(Some(et?))
                }
                None => Ok(None),
            }
        }
    }

    m.add_class::<QuadTreeWrapper>()?;
    m.add_class::<PyCircle>()?;
    m.add_class::<PyRectangle>()?;
    m.add_class::<PyConfig>()?;
    Ok(())
}
