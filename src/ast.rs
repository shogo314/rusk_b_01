use crate::rusk_grammar_trait as g;
use parol_runtime::Token;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Span {
    pub file: String,
    pub line: u32,
    pub column: u32,
}
impl Span {
    pub fn token(t: &Token<'_>) -> Self {
        Self {
            file: t.location.file_name.display().to_string(),
            line: t.location.start_line,
            column: t.location.start_column,
        }
    }
    pub fn error(&self, message: impl std::fmt::Display) -> String {
        format!("{}:{}:{}: {}", self.file, self.line, self.column, message)
    }
}
#[derive(Clone, Debug)]
pub struct Name {
    pub text: String,
    pub span: Span,
}
fn name(n: &g::Identifier<'_>) -> Name {
    Name {
        text: n.identifier.text().into(),
        span: Span::token(&n.identifier),
    }
}
fn type_name(t: &g::TypeName<'_>) -> Name {
    let mut n = name(&t.identifier);
    if let Some(arg) = &t.type_name_opt {
        n.text = format!("{}<{}>", n.text, type_name(&arg.type_name).text);
    }
    n
}
#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub enum ExprKind {
    Literal(String, String),
    Array(Vec<Expr>),
    Repeat(Box<Expr>, Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Var(Name),
    Call(Name, Vec<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Unary(String, Box<Expr>),
    Assign(String, Box<Expr>, Box<Expr>),
}
#[derive(Clone, Debug)]
pub enum Stmt {
    Type(Name, BTreeMap<String, String>),
    Function(
        Name,
        Vec<(Name, Name)>,
        Option<Name>,
        BTreeMap<String, String>,
    ),
    UserFunction(Name, Vec<(Name, Name)>, Option<Name>, Box<Stmt>),
    Return(Span, Option<Expr>),
    Symbol(Name, BTreeMap<String, String>),
    Let(Name, Option<Name>, Option<Expr>),
    Input(Vec<(Name, Name)>),
    Output(Vec<Expr>),
    Block(Vec<Stmt>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>, Option<Box<Stmt>>),
    Loop(Box<Stmt>),
    Break(Span),
    Continue(Span),
    Expr(Expr),
}
fn props(p: &g::Properties<'_>) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for entry in &p.properties_list {
        let p = &entry.property;
        let n = name(&p.identifier);
        let value = serde_json::from_str(p.string_literal.string_literal.text())
            .map_err(|e| n.span.error(e))?;
        if result.insert(n.text.clone(), value).is_some() {
            return Err(n.span.error(format!("duplicate property '{}'", n.text)));
        }
    }
    Ok(result)
}
pub fn lower(root: &g::Rusk<'_>) -> Result<Vec<Stmt>, String> {
    root.rusk_list.iter().map(|s| stmt(&s.statement)).collect()
}
fn stmt(s: &g::Statement<'_>) -> Result<Stmt, String> {
    Ok(match s {
        g::Statement::ExternType(x) => Stmt::Type(
            type_name(&x.extern_type.type_name),
            props(&x.extern_type.properties)?,
        ),
        g::Statement::ExternSymbol(x) => Stmt::Symbol(
            name(&x.extern_symbol.identifier),
            props(&x.extern_symbol.properties)?,
        ),
        g::Statement::ExternFn(x) => {
            let x = &x.extern_fn;
            let mut params = vec![];
            if let Some(p) = &x.parameters.parameters_opt {
                params.push((
                    name(&p.parameter.identifier),
                    type_name(&p.parameter.type_name),
                ));
                params.extend(p.parameters_opt_list.iter().map(|p| {
                    (
                        name(&p.parameter.identifier),
                        type_name(&p.parameter.type_name),
                    )
                }));
            }
            Stmt::Function(
                name(&x.identifier),
                params,
                x.extern_fn_opt.as_ref().map(|r| type_name(&r.type_name)),
                props(&x.properties)?,
            )
        }
        g::Statement::Function(x) => {
            let x = &x.function;
            let mut params = vec![];
            if let Some(p) = &x.parameters.parameters_opt {
                params.push((
                    name(&p.parameter.identifier),
                    type_name(&p.parameter.type_name),
                ));
                params.extend(p.parameters_opt_list.iter().map(|p| {
                    (
                        name(&p.parameter.identifier),
                        type_name(&p.parameter.type_name),
                    )
                }));
            }
            Stmt::UserFunction(
                name(&x.identifier),
                params,
                x.function_opt.as_ref().map(|r| type_name(&r.type_name)),
                Box::new(block(&x.block)?),
            )
        }
        g::Statement::Return(x) => Stmt::Return(
            Span::token(&x.r#return.r#return),
            x.r#return
                .return_opt
                .as_ref()
                .map(|x| expr(&x.expr))
                .transpose()?,
        ),
        g::Statement::Let(x) => {
            let x = &x.r#let;
            Stmt::Let(
                name(&x.identifier),
                x.let_opt.as_ref().map(|t| type_name(&t.type_name)),
                x.let_opt0.as_ref().map(|e| expr(&e.expr)).transpose()?,
            )
        }
        g::Statement::Input(x) => {
            let mut items = vec![];
            for item in std::iter::once(x.input.input_item.as_ref())
                .chain(x.input.input_list.iter().map(|i| i.input_item.as_ref()))
            {
                let ty = type_name(&item.type_name);
                items.push((name(&item.identifier), ty.clone()));
                items.extend(
                    item.input_item_list
                        .iter()
                        .map(|i| (name(&i.identifier), ty.clone())),
                );
            }
            Stmt::Input(items)
        }
        g::Statement::Output(x) => Stmt::Output(list(&x.output.expr_list)?),
        g::Statement::Block(x) => block(&x.block)?,
        g::Statement::If(x) => Stmt::If(
            expr(&x.r#if.expr)?,
            Box::new(block(&x.r#if.block)?),
            x.r#if
                .if_opt
                .as_ref()
                .map(|x| block(&x.block).map(Box::new))
                .transpose()?,
        ),
        g::Statement::While(x) => Stmt::While(
            expr(&x.r#while.expr)?,
            Box::new(block(&x.r#while.block)?),
            x.r#while
                .while_opt
                .as_ref()
                .map(|x| block(&x.block).map(Box::new))
                .transpose()?,
        ),
        g::Statement::Loop(x) => Stmt::Loop(Box::new(block(&x.r#loop.block)?)),
        g::Statement::Break(x) => Stmt::Break(Span::token(&x.r#break.r#break)),
        g::Statement::Continue(x) => Stmt::Continue(Span::token(&x.r#continue.r#continue)),
        g::Statement::ExprSemicolon(x) => Stmt::Expr(expr(&x.expr)?),
    })
}
fn block(b: &g::Block<'_>) -> Result<Stmt, String> {
    Ok(Stmt::Block(
        b.block_list
            .iter()
            .map(|s| stmt(&s.statement))
            .collect::<Result<_, _>>()?,
    ))
}
fn list(l: &g::ExprList<'_>) -> Result<Vec<Expr>, String> {
    let Some(x) = &l.expr_list_opt else {
        return Ok(vec![]);
    };
    std::iter::once(x.expr.as_ref())
        .chain(x.expr_list_opt_list.iter().map(|i| i.expr.as_ref()))
        .map(expr)
        .collect()
}
fn expr(e: &g::Expr<'_>) -> Result<Expr, String> {
    let left = compare(&e.compare)?;
    if let Some(x) = &e.expr_opt {
        let (op, t) = match x.assign_op.as_ref() {
            g::AssignOp::ColonEqu(x) => (":=", &x.colon_equ),
            g::AssignOp::Equ(x) => ("=", &x.equ),
            g::AssignOp::PlusEqu(x) => ("+=", &x.plus_equ),
            g::AssignOp::MinusEqu(x) => ("-=", &x.minus_equ),
            g::AssignOp::StarEqu(x) => ("*=", &x.star_equ),
            g::AssignOp::SlashEqu(x) => ("/=", &x.slash_equ),
            g::AssignOp::PercentEqu(x) => ("%=", &x.percent_equ),
        };
        return Ok(Expr {
            kind: ExprKind::Assign(op.into(), Box::new(left), Box::new(expr(&x.expr)?)),
            span: Span::token(t),
        });
    }
    Ok(left)
}
fn compare(e: &g::Compare<'_>) -> Result<Expr, String> {
    let left = add(&e.add)?;
    let Some(x) = &e.compare_opt else {
        return Ok(left);
    };
    let (op, t) = match x.compare_op.as_ref() {
        g::CompareOp::EquEqu(x) => ("==", &x.equ_equ),
        g::CompareOp::BangEqu(x) => ("!=", &x.bang_equ),
        g::CompareOp::LT(x) => ("<", &x.l_t),
        g::CompareOp::LTEqu(x) => ("<=", &x.l_t_equ),
        g::CompareOp::GT(x) => (">", &x.g_t),
        g::CompareOp::GTEqu(x) => (">=", &x.g_t_equ),
    };
    Ok(Expr {
        kind: ExprKind::Binary(op.into(), Box::new(left), Box::new(add(&x.add)?)),
        span: Span::token(t),
    })
}
fn add(e: &g::Add<'_>) -> Result<Expr, String> {
    let mut left = mul(&e.mul)?;
    for x in &e.add_list {
        let (op, t) = match x.add_op.as_ref() {
            g::AddOp::Plus(x) => ("+", &x.plus),
            g::AddOp::Minus(x) => ("-", &x.minus),
        };
        left = Expr {
            kind: ExprKind::Binary(op.into(), Box::new(left), Box::new(mul(&x.mul)?)),
            span: Span::token(t),
        };
    }
    Ok(left)
}
fn mul(e: &g::Mul<'_>) -> Result<Expr, String> {
    let mut left = unary(&e.unary)?;
    for x in &e.mul_list {
        let (op, t) = match x.mul_op.as_ref() {
            g::MulOp::Star(x) => ("*", &x.star),
            g::MulOp::Slash(x) => ("/", &x.slash),
            g::MulOp::Percent(x) => ("%", &x.percent),
        };
        left = Expr {
            kind: ExprKind::Binary(op.into(), Box::new(left), Box::new(unary(&x.unary)?)),
            span: Span::token(t),
        };
    }
    Ok(left)
}
fn unary(e: &g::Unary<'_>) -> Result<Expr, String> {
    match e {
        g::Unary::UnaryOpUnary(x) => {
            let (op, t) = match x.unary_op.as_ref() {
                g::UnaryOp::Plus(x) => ("+", &x.plus),
                g::UnaryOp::Minus(x) => ("-", &x.minus),
            };
            Ok(Expr {
                kind: ExprKind::Unary(op.into(), Box::new(unary(&x.unary)?)),
                span: Span::token(t),
            })
        }
        g::Unary::Postfix(x) => postfix(&x.postfix),
    }
}
fn postfix(p: &g::Postfix<'_>) -> Result<Expr, String> {
    let mut value = primary(&p.primary)?;
    for suffix in &p.postfix_list {
        value = match suffix.suffix.as_ref() {
            g::Suffix::Index(x) => Expr {
                kind: ExprKind::Index(Box::new(value), Box::new(expr(&x.index.expr)?)),
                span: Span::token(&x.index.l_bracket),
            },
            g::Suffix::Method(x) => {
                let n = name(&x.method.identifier);
                let mut args = vec![value];
                args.extend(list(&x.method.expr_list)?);
                Expr {
                    span: n.span.clone(),
                    kind: ExprKind::Call(n, args),
                }
            }
        };
    }
    Ok(value)
}
fn primary(e: &g::Primary<'_>) -> Result<Expr, String> {
    Ok(match e {
        g::Primary::Array(x) => {
            let kind = if let Some(items) = &x.array.array_opt {
                let first = expr(&items.expr)?;
                match items.array_tail.as_ref() {
                    g::ArrayTail::SemicolonExpr(x) => {
                        ExprKind::Repeat(Box::new(first), Box::new(expr(&x.expr)?))
                    }
                    g::ArrayTail::ArrayTailList(x) => {
                        let mut values = vec![first];
                        values.extend(
                            x.array_tail_list
                                .iter()
                                .map(|x| expr(&x.expr))
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                        ExprKind::Array(values)
                    }
                }
            } else {
                ExprKind::Array(vec![])
            };
            Expr {
                kind,
                span: Span::token(&x.array.l_bracket),
            }
        }
        g::Primary::LParenExprRParen(x) => return expr(&x.expr),
        g::Primary::IdentifierPrimaryOpt(x) => {
            let n = name(&x.identifier);
            Expr {
                span: n.span.clone(),
                kind: if let Some(args) = &x.primary_opt {
                    ExprKind::Call(n, list(&args.expr_list)?)
                } else {
                    ExprKind::Var(n)
                },
            }
        }
        g::Primary::Literal(x) => {
            let (ty, t) = match x.literal.as_ref() {
                g::Literal::IntLiteral(x) => ("Int", &x.int_literal.int_literal),
                g::Literal::StringLiteral(x) => ("Str", &x.string_literal.string_literal),
                g::Literal::CharLiteral(x) => ("Char", &x.char_literal.char_literal),
                g::Literal::BoolLiteral(x) => (
                    "Bool",
                    match x.bool_literal.as_ref() {
                        g::BoolLiteral::True(x) => &x.r#true,
                        g::BoolLiteral::False(x) => &x.r#false,
                    },
                ),
            };
            Expr {
                kind: ExprKind::Literal(ty.into(), t.text().into()),
                span: Span::token(t),
            }
        }
    })
}
