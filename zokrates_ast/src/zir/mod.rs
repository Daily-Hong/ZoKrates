pub mod folder;
mod from_typed;
mod identifier;
pub mod lqc;
mod parameter;
pub mod result_folder;
pub mod types;
mod uint;
mod variable;

pub use self::parameter::Parameter;
pub use self::types::{Type, UBitwidth};
pub use self::variable::Variable;
use crate::common::{FlatEmbed, FormatString, SourceMetadata};
use crate::typed::ConcreteType;
pub use crate::zir::uint::{ShouldReduce, UExpression, UExpressionInner, UMetadata};

use crate::zir::types::Signature;
use std::fmt;
use std::marker::PhantomData;
use zokrates_field::Field;

pub use self::folder::Folder;
pub use self::identifier::{Identifier, SourceIdentifier};
use serde::{Deserialize, Serialize};

/// A typed program as a collection of modules, one of them being the main
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct ZirProgram<'ast, T> {
    pub main: ZirFunction<'ast, T>,
}

impl<'ast, T: fmt::Display> fmt::Display for ZirProgram<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.main)
    }
}
/// A typed function
#[derive(Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct ZirFunction<'ast, T> {
    /// Arguments of the function
    #[serde(borrow)]
    pub arguments: Vec<Parameter<'ast>>,
    /// Vector of statements that are executed when running the function
    #[serde(borrow)]
    pub statements: Vec<ZirStatement<'ast, T>>,
    /// function signature
    pub signature: Signature,
}

impl<'ast, T: fmt::Display> fmt::Display for ZirFunction<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "({}) -> ({}) {{",
            self.arguments
                .iter()
                .map(|x| format!("{}", x))
                .collect::<Vec<_>>()
                .join(", "),
            self.signature
                .outputs
                .iter()
                .map(|x| format!("{}", x))
                .collect::<Vec<_>>()
                .join(", ")
        )?;

        for s in &self.statements {
            s.fmt_indented(f, 1)?;
            writeln!(f)?;
        }

        write!(f, "}}")
    }
}

impl<'ast, T: fmt::Debug> fmt::Debug for ZirFunction<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ZirFunction(arguments: {:?}, ...):\n{}",
            self.arguments,
            self.statements
                .iter()
                .map(|x| format!("\t{:?}", x))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

pub type ZirAssignee<'ast> = Variable<'ast>;

#[derive(Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub enum RuntimeError {
    SourceAssertion(SourceMetadata),
    SelectRangeCheck,
    DivisionByZero,
    IncompleteDynamicRange,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::SourceAssertion(metadata) => write!(f, "{}", metadata),
            RuntimeError::SelectRangeCheck => write!(f, "Range check on array access"),
            RuntimeError::DivisionByZero => write!(f, "Division by zero"),
            RuntimeError::IncompleteDynamicRange => write!(f, "Dynamic comparison is incomplete"),
        }
    }
}

impl RuntimeError {
    pub fn mock() -> Self {
        RuntimeError::SourceAssertion(SourceMetadata::default())
    }
}

#[derive(Clone, PartialEq, Hash, Eq, Debug, Serialize, Deserialize)]
pub enum ZirAssemblyStatement<'ast, T> {
    Assignment(
        #[serde(borrow)] Vec<ZirAssignee<'ast>>,
        ZirFunction<'ast, T>,
    ),
    Constraint(
        FieldElementExpression<'ast, T>,
        FieldElementExpression<'ast, T>,
        SourceMetadata,
    ),
}

impl<'ast, T: fmt::Display> fmt::Display for ZirAssemblyStatement<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ZirAssemblyStatement::Assignment(ref lhs, ref rhs) => {
                write!(
                    f,
                    "{} <-- {};",
                    lhs.iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    rhs
                )
            }
            ZirAssemblyStatement::Constraint(ref lhs, ref rhs, _) => {
                write!(f, "{} === {};", lhs, rhs)
            }
        }
    }
}

/// A statement in a `ZirFunction`
#[derive(Clone, PartialEq, Hash, Eq, Debug, Serialize, Deserialize)]
pub enum ZirStatement<'ast, T> {
    Return(Vec<ZirExpression<'ast, T>>),
    Definition(ZirAssignee<'ast>, ZirExpression<'ast, T>),
    IfElse(
        BooleanExpression<'ast, T>,
        Vec<ZirStatement<'ast, T>>,
        Vec<ZirStatement<'ast, T>>,
    ),
    Assertion(BooleanExpression<'ast, T>, RuntimeError),
    MultipleDefinition(Vec<ZirAssignee<'ast>>, ZirExpressionList<'ast, T>),
    Log(
        FormatString,
        Vec<(ConcreteType, Vec<ZirExpression<'ast, T>>)>,
    ),
    #[serde(borrow)]
    Assembly(Vec<ZirAssemblyStatement<'ast, T>>),
}

impl<'ast, T: fmt::Display> fmt::Display for ZirStatement<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_indented(f, 0)
    }
}

impl<'ast, T: fmt::Display> ZirStatement<'ast, T> {
    fn fmt_indented(&self, f: &mut fmt::Formatter, depth: usize) -> fmt::Result {
        write!(f, "{}", "\t".repeat(depth))?;
        match self {
            ZirStatement::Return(ref exprs) => {
                write!(f, "return")?;
                if !exprs.is_empty() {
                    write!(
                        f,
                        " {}",
                        exprs
                            .iter()
                            .map(|e| e.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )?;
                }
                write!(f, ";")
            }
            ZirStatement::Definition(ref lhs, ref rhs) => {
                write!(f, "{} = {};", lhs, rhs)
            }
            ZirStatement::IfElse(ref condition, ref consequence, ref alternative) => {
                writeln!(f, "if {} {{", condition)?;
                for s in consequence {
                    s.fmt_indented(f, depth + 1)?;
                    writeln!(f)?;
                }
                writeln!(f, "{}}} else {{", "\t".repeat(depth))?;
                for s in alternative {
                    s.fmt_indented(f, depth + 1)?;
                    writeln!(f)?;
                }
                write!(f, "{}}}", "\t".repeat(depth))
            }
            ZirStatement::Assertion(ref e, ref error) => {
                write!(f, "assert({}", e)?;
                match error {
                    RuntimeError::SourceAssertion(message) => write!(f, ", \"{}\");", message),
                    error => write!(f, "); // {}", error),
                }
            }
            ZirStatement::MultipleDefinition(ref ids, ref rhs) => {
                for (i, id) in ids.iter().enumerate() {
                    write!(f, "{}", id)?;
                    if i < ids.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, " = {};", rhs)
            }
            ZirStatement::Log(ref l, ref expressions) => write!(
                f,
                "log(\"{}\"), {});",
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
            ZirStatement::Assembly(statements) => {
                writeln!(f, "asm {{")?;
                for s in statements {
                    writeln!(f, "{}{}", "\t".repeat(depth + 1), s)?;
                }
                write!(f, "{}}}", "\t".repeat(depth))
            }
        }
    }
}

pub trait Typed {
    fn get_type(&self) -> Type;
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct IdentifierExpression<'ast, E> {
    #[serde(borrow)]
    pub id: Identifier<'ast>,
    ty: PhantomData<E>,
}

impl<'ast, E> fmt::Display for IdentifierExpression<'ast, E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl<'ast, E> IdentifierExpression<'ast, E> {
    pub fn new(id: Identifier<'ast>) -> Self {
        IdentifierExpression {
            id,
            ty: PhantomData,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct ConditionalExpression<'ast, T, E> {
    #[serde(borrow)]
    pub condition: Box<BooleanExpression<'ast, T>>,
    pub consequence: Box<E>,
    pub alternative: Box<E>,
}

impl<'ast, T, E> ConditionalExpression<'ast, T, E> {
    pub fn new(condition: BooleanExpression<'ast, T>, consequence: E, alternative: E) -> Self {
        ConditionalExpression {
            condition: box condition,
            consequence: box consequence,
            alternative: box alternative,
        }
    }
}

impl<'ast, T: fmt::Display, E: fmt::Display> fmt::Display for ConditionalExpression<'ast, T, E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} ? {} : {}",
            self.condition, self.consequence, self.alternative
        )
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct SelectExpression<'ast, T, E> {
    pub array: Vec<E>,
    #[serde(borrow)]
    pub index: Box<UExpression<'ast, T>>,
}

impl<'ast, T, E> SelectExpression<'ast, T, E> {
    pub fn new(array: Vec<E>, index: UExpression<'ast, T>) -> Self {
        SelectExpression {
            array,
            index: box index,
        }
    }
}

impl<'ast, T: fmt::Display, E: fmt::Display> fmt::Display for SelectExpression<'ast, T, E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}[{}]",
            self.array
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            self.index
        )
    }
}

/// A typed expression
#[derive(Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub enum ZirExpression<'ast, T> {
    Boolean(BooleanExpression<'ast, T>),
    FieldElement(FieldElementExpression<'ast, T>),
    Uint(#[serde(borrow)] UExpression<'ast, T>),
}

impl<'ast, T: Field> From<BooleanExpression<'ast, T>> for ZirExpression<'ast, T> {
    fn from(e: BooleanExpression<'ast, T>) -> ZirExpression<T> {
        ZirExpression::Boolean(e)
    }
}

impl<'ast, T: Field> From<FieldElementExpression<'ast, T>> for ZirExpression<'ast, T> {
    fn from(e: FieldElementExpression<'ast, T>) -> ZirExpression<T> {
        ZirExpression::FieldElement(e)
    }
}

impl<'ast, T: Field> From<UExpression<'ast, T>> for ZirExpression<'ast, T> {
    fn from(e: UExpression<'ast, T>) -> ZirExpression<T> {
        ZirExpression::Uint(e)
    }
}

impl<'ast, T: fmt::Display> fmt::Display for ZirExpression<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ZirExpression::Boolean(ref e) => write!(f, "{}", e),
            ZirExpression::FieldElement(ref e) => write!(f, "{}", e),
            ZirExpression::Uint(ref e) => write!(f, "{}", e),
        }
    }
}

impl<'ast, T: fmt::Debug> fmt::Debug for ZirExpression<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ZirExpression::Boolean(ref e) => write!(f, "{:?}", e),
            ZirExpression::FieldElement(ref e) => write!(f, "{:?}", e),
            ZirExpression::Uint(ref e) => write!(f, "{:?}", e),
        }
    }
}

impl<'ast, T: Field> Typed for ZirExpression<'ast, T> {
    fn get_type(&self) -> Type {
        match *self {
            ZirExpression::Boolean(ref e) => e.get_type(),
            ZirExpression::FieldElement(ref e) => e.get_type(),
            ZirExpression::Uint(ref e) => e.get_type(),
        }
    }
}

impl<'ast, T: Field> Typed for FieldElementExpression<'ast, T> {
    fn get_type(&self) -> Type {
        Type::FieldElement
    }
}

impl<'ast, T: Field> Typed for UExpression<'ast, T> {
    fn get_type(&self) -> Type {
        Type::Uint(self.bitwidth)
    }
}

impl<'ast, T: Field> Typed for BooleanExpression<'ast, T> {
    fn get_type(&self) -> Type {
        Type::Boolean
    }
}

pub trait MultiTyped {
    fn get_types(&self) -> &Vec<Type>;
}

#[derive(Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub enum ZirExpressionList<'ast, T> {
    EmbedCall(
        FlatEmbed,
        Vec<u32>,
        #[serde(borrow)] Vec<ZirExpression<'ast, T>>,
    ),
}

/// An expression of type `field`
#[derive(Clone, PartialEq, Hash, Eq, Debug, Serialize, Deserialize)]
pub enum FieldElementExpression<'ast, T> {
    Number(T),
    #[serde(borrow)]
    Identifier(IdentifierExpression<'ast, Self>),
    Select(SelectExpression<'ast, T, Self>),
    Add(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    Sub(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    Mult(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    Div(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    Pow(
        Box<FieldElementExpression<'ast, T>>,
        #[serde(borrow)] Box<UExpression<'ast, T>>,
    ),
    And(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    Or(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    Xor(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    LeftShift(
        Box<FieldElementExpression<'ast, T>>,
        Box<UExpression<'ast, T>>,
    ),
    RightShift(
        Box<FieldElementExpression<'ast, T>>,
        Box<UExpression<'ast, T>>,
    ),
    Conditional(ConditionalExpression<'ast, T, FieldElementExpression<'ast, T>>),
}

impl<'ast, T> FieldElementExpression<'ast, T> {
    pub fn is_linear(&self) -> bool {
        match self {
            FieldElementExpression::Number(_) => true,
            FieldElementExpression::Identifier(_) => true,
            FieldElementExpression::Add(box left, box right) => {
                left.is_linear() && right.is_linear()
            }
            FieldElementExpression::Sub(box left, box right) => {
                left.is_linear() && right.is_linear()
            }
            FieldElementExpression::Mult(box left, box right) => matches!(
                (left, right),
                (FieldElementExpression::Number(_), _) | (_, FieldElementExpression::Number(_))
            ),
            _ => false,
        }
    }
}

/// An expression of type `bool`
#[derive(Clone, PartialEq, Hash, Eq, Debug, Serialize, Deserialize)]
pub enum BooleanExpression<'ast, T> {
    Value(bool),
    #[serde(borrow)]
    Identifier(IdentifierExpression<'ast, Self>),
    Select(SelectExpression<'ast, T, Self>),
    FieldLt(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    FieldLe(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    FieldEq(
        Box<FieldElementExpression<'ast, T>>,
        Box<FieldElementExpression<'ast, T>>,
    ),
    UintLt(Box<UExpression<'ast, T>>, Box<UExpression<'ast, T>>),
    UintLe(Box<UExpression<'ast, T>>, Box<UExpression<'ast, T>>),
    UintEq(Box<UExpression<'ast, T>>, Box<UExpression<'ast, T>>),
    BoolEq(
        Box<BooleanExpression<'ast, T>>,
        Box<BooleanExpression<'ast, T>>,
    ),
    Or(
        Box<BooleanExpression<'ast, T>>,
        Box<BooleanExpression<'ast, T>>,
    ),
    And(
        Box<BooleanExpression<'ast, T>>,
        Box<BooleanExpression<'ast, T>>,
    ),
    Not(Box<BooleanExpression<'ast, T>>),
    Conditional(ConditionalExpression<'ast, T, BooleanExpression<'ast, T>>),
}

pub struct ConjunctionIterator<T> {
    current: Vec<T>,
}

impl<'ast, T> Iterator for ConjunctionIterator<BooleanExpression<'ast, T>> {
    type Item = BooleanExpression<'ast, T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.pop().and_then(|n| match n {
            BooleanExpression::And(box left, box right) => {
                self.current.push(left);
                self.current.push(right);
                self.next()
            }
            n => Some(n),
        })
    }
}

impl<'ast, T> BooleanExpression<'ast, T> {
    pub fn into_conjunction_iterator(self) -> ConjunctionIterator<Self> {
        ConjunctionIterator {
            current: vec![self],
        }
    }
}

// Downcasts
impl<'ast, T> From<ZirExpression<'ast, T>> for FieldElementExpression<'ast, T> {
    fn from(e: ZirExpression<'ast, T>) -> FieldElementExpression<'ast, T> {
        match e {
            ZirExpression::FieldElement(e) => e,
            _ => unreachable!("downcast failed"),
        }
    }
}

impl<'ast, T> From<ZirExpression<'ast, T>> for BooleanExpression<'ast, T> {
    fn from(e: ZirExpression<'ast, T>) -> BooleanExpression<'ast, T> {
        match e {
            ZirExpression::Boolean(e) => e,
            _ => unreachable!("downcast failed"),
        }
    }
}

impl<'ast, T> From<ZirExpression<'ast, T>> for UExpression<'ast, T> {
    fn from(e: ZirExpression<'ast, T>) -> UExpression<'ast, T> {
        match e {
            ZirExpression::Uint(e) => e,
            _ => unreachable!("downcast failed"),
        }
    }
}

impl<'ast, T: fmt::Display> fmt::Display for FieldElementExpression<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FieldElementExpression::Number(ref i) => write!(f, "{}", i),
            FieldElementExpression::Identifier(ref var) => write!(f, "{}", var),
            FieldElementExpression::Select(ref e) => write!(f, "{}", e),
            FieldElementExpression::Add(ref lhs, ref rhs) => write!(f, "({} + {})", lhs, rhs),
            FieldElementExpression::Sub(ref lhs, ref rhs) => write!(f, "({} - {})", lhs, rhs),
            FieldElementExpression::Mult(ref lhs, ref rhs) => write!(f, "({} * {})", lhs, rhs),
            FieldElementExpression::Div(ref lhs, ref rhs) => write!(f, "({} / {})", lhs, rhs),
            FieldElementExpression::Pow(ref lhs, ref rhs) => write!(f, "{}**{}", lhs, rhs),
            FieldElementExpression::And(ref lhs, ref rhs) => write!(f, "({} & {})", lhs, rhs),
            FieldElementExpression::Or(ref lhs, ref rhs) => write!(f, "({} | {})", lhs, rhs),
            FieldElementExpression::Xor(ref lhs, ref rhs) => write!(f, "({} ^ {})", lhs, rhs),
            FieldElementExpression::LeftShift(ref lhs, ref rhs) => {
                write!(f, "({} << {})", lhs, rhs)
            }
            FieldElementExpression::RightShift(ref lhs, ref rhs) => {
                write!(f, "({} >> {})", lhs, rhs)
            }
            FieldElementExpression::Conditional(ref c) => {
                write!(f, "{}", c)
            }
        }
    }
}

impl<'ast, T: fmt::Display> fmt::Display for UExpression<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.inner {
            UExpressionInner::Value(ref v) => write!(f, "{}", v),
            UExpressionInner::Identifier(ref var) => write!(f, "{}", var),
            UExpressionInner::Select(ref e) => write!(f, "{}", e),
            UExpressionInner::Add(ref lhs, ref rhs) => write!(f, "({} + {})", lhs, rhs),
            UExpressionInner::Sub(ref lhs, ref rhs) => write!(f, "({} - {})", lhs, rhs),
            UExpressionInner::Mult(ref lhs, ref rhs) => write!(f, "({} * {})", lhs, rhs),
            UExpressionInner::Div(ref lhs, ref rhs) => write!(f, "({} * {})", lhs, rhs),
            UExpressionInner::Rem(ref lhs, ref rhs) => write!(f, "({} % {})", lhs, rhs),
            UExpressionInner::Xor(ref lhs, ref rhs) => write!(f, "({} ^ {})", lhs, rhs),
            UExpressionInner::And(ref lhs, ref rhs) => write!(f, "({} & {})", lhs, rhs),
            UExpressionInner::Or(ref lhs, ref rhs) => write!(f, "({} | {})", lhs, rhs),
            UExpressionInner::LeftShift(ref e, ref by) => write!(f, "({} << {})", e, by),
            UExpressionInner::RightShift(ref e, ref by) => write!(f, "({} >> {})", e, by),
            UExpressionInner::Not(ref e) => write!(f, "!{}", e),
            UExpressionInner::Conditional(ref c) => {
                write!(f, "{}", c)
            }
        }
    }
}

impl<'ast, T: fmt::Display> fmt::Display for BooleanExpression<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BooleanExpression::Identifier(ref var) => write!(f, "{}", var),
            BooleanExpression::Value(b) => write!(f, "{}", b),
            BooleanExpression::Select(ref e) => write!(f, "{}", e),
            BooleanExpression::FieldLt(ref lhs, ref rhs) => write!(f, "({} < {})", lhs, rhs),
            BooleanExpression::UintLt(ref lhs, ref rhs) => write!(f, "({} < {})", lhs, rhs),
            BooleanExpression::FieldLe(ref lhs, ref rhs) => write!(f, "({} <= {})", lhs, rhs),
            BooleanExpression::UintLe(ref lhs, ref rhs) => write!(f, "({} <= {})", lhs, rhs),
            BooleanExpression::FieldEq(ref lhs, ref rhs) => write!(f, "({} == {})", lhs, rhs),
            BooleanExpression::BoolEq(ref lhs, ref rhs) => write!(f, "({} == {})", lhs, rhs),
            BooleanExpression::UintEq(ref lhs, ref rhs) => write!(f, "({} == {})", lhs, rhs),
            BooleanExpression::Or(ref lhs, ref rhs) => write!(f, "({} || {})", lhs, rhs),
            BooleanExpression::And(ref lhs, ref rhs) => write!(f, "({} && {})", lhs, rhs),
            BooleanExpression::Not(ref exp) => write!(f, "!{}", exp),
            BooleanExpression::Conditional(ref c) => {
                write!(f, "{}", c)
            }
        }
    }
}

impl<'ast, T: fmt::Display> fmt::Display for ZirExpressionList<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ZirExpressionList::EmbedCall(ref embed, ref generics, ref p) => {
                write!(
                    f,
                    "{}{}(",
                    embed.id(),
                    if generics.is_empty() {
                        "".into()
                    } else {
                        format!(
                            "::<{}>",
                            generics
                                .iter()
                                .map(|g| g.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                )?;
                for (i, param) in p.iter().enumerate() {
                    write!(f, "{}", param)?;
                    if i < p.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ")")
            }
        }
    }
}

impl<'ast, T: fmt::Debug> fmt::Debug for ZirExpressionList<'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ZirExpressionList::EmbedCall(ref embed, ref generics, ref p) => {
                write!(f, "EmbedCall({:?}, {:?}, (", generics, embed)?;
                f.debug_list().entries(p.iter()).finish()?;
                write!(f, ")")
            }
        }
    }
}

// Common behaviour across expressions
pub trait Expr<'ast, T>: fmt::Display + PartialEq + From<ZirExpression<'ast, T>> {
    type Inner;
    type Ty: Clone + IntoType;

    fn ty(&self) -> &Self::Ty;

    fn into_inner(self) -> Self::Inner;

    fn as_inner(&self) -> &Self::Inner;

    fn as_inner_mut(&mut self) -> &mut Self::Inner;
}

impl<'ast, T: Field> Expr<'ast, T> for FieldElementExpression<'ast, T> {
    type Inner = Self;
    type Ty = Type;

    fn ty(&self) -> &Self::Ty {
        &Type::FieldElement
    }

    fn into_inner(self) -> Self::Inner {
        self
    }

    fn as_inner(&self) -> &Self::Inner {
        self
    }

    fn as_inner_mut(&mut self) -> &mut Self::Inner {
        self
    }
}

impl<'ast, T: Field> Expr<'ast, T> for BooleanExpression<'ast, T> {
    type Inner = Self;
    type Ty = Type;

    fn ty(&self) -> &Self::Ty {
        &Type::Boolean
    }

    fn into_inner(self) -> Self::Inner {
        self
    }

    fn as_inner(&self) -> &Self::Inner {
        self
    }

    fn as_inner_mut(&mut self) -> &mut Self::Inner {
        self
    }
}

impl<'ast, T: Field> Expr<'ast, T> for UExpression<'ast, T> {
    type Inner = UExpressionInner<'ast, T>;
    type Ty = UBitwidth;

    fn ty(&self) -> &Self::Ty {
        &self.bitwidth
    }

    fn into_inner(self) -> Self::Inner {
        self.inner
    }

    fn as_inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn as_inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.inner
    }
}

pub trait Id<'ast, T>: Expr<'ast, T> {
    fn identifier(id: Identifier<'ast>) -> Self::Inner;
}

impl<'ast, T: Field> Id<'ast, T> for FieldElementExpression<'ast, T> {
    fn identifier(id: Identifier<'ast>) -> Self::Inner {
        FieldElementExpression::Identifier(IdentifierExpression::new(id))
    }
}

impl<'ast, T: Field> Id<'ast, T> for BooleanExpression<'ast, T> {
    fn identifier(id: Identifier<'ast>) -> Self::Inner {
        BooleanExpression::Identifier(IdentifierExpression::new(id))
    }
}

impl<'ast, T: Field> Id<'ast, T> for UExpression<'ast, T> {
    fn identifier(id: Identifier<'ast>) -> Self::Inner {
        UExpressionInner::Identifier(IdentifierExpression::new(id))
    }
}

pub enum IdentifierOrExpression<'ast, T, E: Expr<'ast, T>> {
    Identifier(IdentifierExpression<'ast, E>),
    Expression(E::Inner),
}

pub trait Conditional<'ast, T> {
    fn conditional(
        condition: BooleanExpression<'ast, T>,
        consequence: Self,
        alternative: Self,
    ) -> Self;
}

pub enum ConditionalOrExpression<'ast, T, E: Expr<'ast, T>> {
    Conditional(ConditionalExpression<'ast, T, E>),
    Expression(E::Inner),
}

impl<'ast, T> Conditional<'ast, T> for FieldElementExpression<'ast, T> {
    fn conditional(
        condition: BooleanExpression<'ast, T>,
        consequence: Self,
        alternative: Self,
    ) -> Self {
        FieldElementExpression::Conditional(ConditionalExpression::new(
            condition,
            consequence,
            alternative,
        ))
    }
}

impl<'ast, T> Conditional<'ast, T> for BooleanExpression<'ast, T> {
    fn conditional(
        condition: BooleanExpression<'ast, T>,
        consequence: Self,
        alternative: Self,
    ) -> Self {
        BooleanExpression::Conditional(ConditionalExpression::new(
            condition,
            consequence,
            alternative,
        ))
    }
}

impl<'ast, T> Conditional<'ast, T> for UExpression<'ast, T> {
    fn conditional(
        condition: BooleanExpression<'ast, T>,
        consequence: Self,
        alternative: Self,
    ) -> Self {
        let bitwidth = consequence.bitwidth;

        UExpressionInner::Conditional(ConditionalExpression::new(
            condition,
            consequence,
            alternative,
        ))
        .annotate(bitwidth)
    }
}
pub trait Select<'ast, T>: Sized {
    fn select(array: Vec<Self>, index: UExpression<'ast, T>) -> Self;
}

pub enum SelectOrExpression<'ast, T, E: Expr<'ast, T>> {
    Select(SelectExpression<'ast, T, E>),
    Expression(E::Inner),
}

impl<'ast, T> Select<'ast, T> for FieldElementExpression<'ast, T> {
    fn select(array: Vec<Self>, index: UExpression<'ast, T>) -> Self {
        FieldElementExpression::Select(SelectExpression::new(array, index))
    }
}

impl<'ast, T> Select<'ast, T> for BooleanExpression<'ast, T> {
    fn select(array: Vec<Self>, index: UExpression<'ast, T>) -> Self {
        BooleanExpression::Select(SelectExpression::new(array, index))
    }
}

impl<'ast, T> Select<'ast, T> for UExpression<'ast, T> {
    fn select(array: Vec<Self>, index: UExpression<'ast, T>) -> Self {
        let bitwidth = array[0].bitwidth;

        UExpressionInner::Select(SelectExpression::new(array, index)).annotate(bitwidth)
    }
}
pub trait IntoType {
    fn into_type(self) -> Type;
}

impl IntoType for Type {
    fn into_type(self) -> Type {
        self
    }
}

impl IntoType for UBitwidth {
    fn into_type(self) -> Type {
        Type::Uint(self)
    }
}

pub trait Constant: Sized {
    // return whether this is constant
    fn is_constant(&self) -> bool;
}

impl<'ast, T: Field> Constant for ZirExpression<'ast, T> {
    fn is_constant(&self) -> bool {
        match self {
            ZirExpression::FieldElement(e) => e.is_constant(),
            ZirExpression::Boolean(e) => e.is_constant(),
            ZirExpression::Uint(e) => e.is_constant(),
        }
    }
}

impl<'ast, T: Field> Constant for FieldElementExpression<'ast, T> {
    fn is_constant(&self) -> bool {
        matches!(self, FieldElementExpression::Number(..))
    }
}

impl<'ast, T: Field> Constant for BooleanExpression<'ast, T> {
    fn is_constant(&self) -> bool {
        matches!(self, BooleanExpression::Value(..))
    }
}

impl<'ast, T: Field> Constant for UExpression<'ast, T> {
    fn is_constant(&self) -> bool {
        matches!(self.as_inner(), UExpressionInner::Value(..))
    }
}
