//! Module containing structs and enums to represent a program.
//!
//! @file absy.rs
//! @author Dennis Kuhnert <dennis.kuhnert@campus.tu-berlin.de>
//! @author Jacob Eberhardt <jacob.eberhardt@tu-berlin.de>
//! @date 2017

pub mod folder;
pub mod utils;

use crate::common::FormatString;
pub use crate::common::Parameter;
pub use crate::common::RuntimeError;
pub use crate::common::Variable;

pub use utils::{
    flat_expression_from_bits, flat_expression_from_expression_summands,
    flat_expression_from_variable_summands,
};

use crate::common::Solver;
use crate::typed::ConcreteType;
use std::collections::HashMap;
use std::fmt;
use zokrates_field::Field;

pub type FlatProg<'ast, T> = FlatFunction<'ast, T>;

pub type FlatFunction<'ast, T> = FlatFunctionIterator<'ast, T, Vec<FlatStatement<'ast, T>>>;

pub type FlatProgIterator<'ast, T, I> = FlatFunctionIterator<'ast, T, I>;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FlatFunctionIterator<'ast, T, I: IntoIterator<Item = FlatStatement<'ast, T>>> {
    /// Arguments of the function
    pub arguments: Vec<Parameter>,
    /// Vector of statements that are executed when running the function
    pub statements: I,
    /// Number of outputs
    pub return_count: usize,
}

impl<'ast, T, I: IntoIterator<Item = FlatStatement<'ast, T>>> FlatFunctionIterator<'ast, T, I> {
    pub fn collect(self) -> FlatFunction<'ast, T> {
        FlatFunction {
            statements: self.statements.into_iter().collect(),
            arguments: self.arguments,
            return_count: self.return_count,
        }
    }
}

impl<'ast, T: Field> fmt::Display for FlatFunction<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "def main({}) -> {}:\n{}",
            self.arguments
                .iter()
                .map(|x| format!("{}", x))
                .collect::<Vec<_>>()
                .join(","),
            self.return_count,
            self.statements
                .iter()
                .map(|x| format!("\t{}", x))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// Calculates a flattened function based on a R1CS (A, B, C) and returns that flattened function:
/// * The Rank 1 Constraint System (R1CS) is defined as:
/// * `<A,x>*<B,x> = <C,x>` for a witness `x`
/// * Since the matrices in R1CS are usually sparse, the following encoding is used:
/// * For each constraint (i.e., row in the R1CS), only non-zero values are supplied and encoded as a tuple (index, value).
///
/// # Arguments
///
/// * r1cs - R1CS in standard JSON data format

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FlatStatement<'ast, T> {
    Block(Vec<FlatStatement<'ast, T>>),
    Condition(FlatExpression<T>, FlatExpression<T>, RuntimeError),
    Definition(Variable, FlatExpression<T>),
    Directive(FlatDirective<'ast, T>),
    Log(FormatString, Vec<(ConcreteType, Vec<FlatExpression<T>>)>),
}

impl<'ast, T: Field> fmt::Display for FlatStatement<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FlatStatement::Block(ref statements) => {
                writeln!(f, "{{")?;
                for s in statements {
                    writeln!(f, "{}", s)?;
                }
                writeln!(f, "}}")
            }
            FlatStatement::Definition(ref lhs, ref rhs) => write!(f, "{} = {}", lhs, rhs),
            FlatStatement::Condition(ref lhs, ref rhs, ref message) => {
                write!(f, "{} == {} // {}", lhs, rhs, message)
            }
            FlatStatement::Directive(ref d) => write!(f, "{}", d),
            FlatStatement::Log(ref l, ref expressions) => write!(
                f,
                "log(\"{}\"), {})",
                l,
                expressions
                    .iter()
                    .map(|(_, e)| format!(
                        "[{}]",
                        e.iter()
                            .map(|e| e.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl<'ast, T: Field> FlatStatement<'ast, T> {
    pub fn apply_substitution(
        self,
        substitution: &'ast HashMap<Variable, Variable>,
    ) -> FlatStatement<T> {
        match self {
            FlatStatement::Block(statements) => FlatStatement::Block(
                statements
                    .into_iter()
                    .map(|s| s.apply_substitution(substitution))
                    .collect(),
            ),
            FlatStatement::Definition(id, x) => FlatStatement::Definition(
                *id.apply_substitution(substitution),
                x.apply_substitution(substitution),
            ),
            FlatStatement::Condition(x, y, message) => FlatStatement::Condition(
                x.apply_substitution(substitution),
                y.apply_substitution(substitution),
                message,
            ),
            FlatStatement::Directive(d) => {
                let outputs = d
                    .outputs
                    .into_iter()
                    .map(|o| *o.apply_substitution(substitution))
                    .collect();
                let inputs = d
                    .inputs
                    .into_iter()
                    .map(|i| i.apply_substitution(substitution))
                    .collect();

                FlatStatement::Directive(FlatDirective {
                    inputs,
                    outputs,
                    ..d
                })
            }
            FlatStatement::Log(l, e) => FlatStatement::Log(
                l,
                e.into_iter()
                    .map(|(t, e)| {
                        (
                            t,
                            e.into_iter()
                                .map(|e| e.apply_substitution(substitution))
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub struct FlatDirective<'ast, T> {
    pub inputs: Vec<FlatExpression<T>>,
    pub outputs: Vec<Variable>,
    pub solver: Solver<'ast, T>,
}

impl<'ast, T> FlatDirective<'ast, T> {
    pub fn new<E: Into<FlatExpression<T>>>(
        outputs: Vec<Variable>,
        solver: Solver<'ast, T>,
        inputs: Vec<E>,
    ) -> Self {
        let (in_len, out_len) = solver.get_signature();
        assert_eq!(in_len, inputs.len());
        assert_eq!(out_len, outputs.len());
        FlatDirective {
            solver,
            inputs: inputs.into_iter().map(|i| i.into()).collect(),
            outputs,
        }
    }
}

impl<'ast, T: Field> fmt::Display for FlatDirective<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "# {} = {}({})",
            self.outputs
                .iter()
                .map(|o| o.to_string())
                .collect::<Vec<String>>()
                .join(", "),
            self.solver,
            self.inputs
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum FlatExpression<T> {
    Number(T),
    Identifier(Variable),
    Add(Box<FlatExpression<T>>, Box<FlatExpression<T>>),
    Sub(Box<FlatExpression<T>>, Box<FlatExpression<T>>),
    Mult(Box<FlatExpression<T>>, Box<FlatExpression<T>>),
}

impl<T> From<T> for FlatExpression<T> {
    fn from(other: T) -> Self {
        Self::Number(other)
    }
}

impl<T: Field> FlatExpression<T> {
    pub fn apply_substitution(
        self,
        substitution: &HashMap<Variable, Variable>,
    ) -> FlatExpression<T> {
        match self {
            e @ FlatExpression::Number(_) => e,
            FlatExpression::Identifier(id) => {
                FlatExpression::Identifier(*id.apply_substitution(substitution))
            }
            FlatExpression::Add(e1, e2) => FlatExpression::Add(
                box e1.apply_substitution(substitution),
                box e2.apply_substitution(substitution),
            ),
            FlatExpression::Sub(e1, e2) => FlatExpression::Sub(
                box e1.apply_substitution(substitution),
                box e2.apply_substitution(substitution),
            ),
            FlatExpression::Mult(e1, e2) => FlatExpression::Mult(
                box e1.apply_substitution(substitution),
                box e2.apply_substitution(substitution),
            ),
        }
    }

    pub fn is_linear(&self) -> bool {
        match *self {
            FlatExpression::Number(_) | FlatExpression::Identifier(_) => true,
            FlatExpression::Add(ref x, ref y) | FlatExpression::Sub(ref x, ref y) => {
                x.is_linear() && y.is_linear()
            }
            FlatExpression::Mult(ref x, ref y) => matches!(
                (x.clone(), y.clone()),
                (box FlatExpression::Number(_), box FlatExpression::Number(_))
                    | (
                        box FlatExpression::Number(_),
                        box FlatExpression::Identifier(_)
                    )
                    | (
                        box FlatExpression::Identifier(_),
                        box FlatExpression::Number(_)
                    )
            ),
        }
    }
}

impl<T: Field> fmt::Display for FlatExpression<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FlatExpression::Number(ref i) => write!(f, "{}", i),
            FlatExpression::Identifier(ref var) => write!(f, "{}", var),
            FlatExpression::Add(ref lhs, ref rhs) => write!(f, "({} + {})", lhs, rhs),
            FlatExpression::Sub(ref lhs, ref rhs) => write!(f, "({} - {})", lhs, rhs),
            FlatExpression::Mult(ref lhs, ref rhs) => write!(f, "({} * {})", lhs, rhs),
        }
    }
}

impl<T: Field> From<Variable> for FlatExpression<T> {
    fn from(v: Variable) -> FlatExpression<T> {
        FlatExpression::Identifier(v)
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct Error {
    message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
