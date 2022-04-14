// Copyright 2022 Singularity Data
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use risingwave_pb::plan::JoinType;

use super::super::plan_node::*;
use super::{BoxedRule, Rule};
use crate::expr::{ExprImpl, ExprRewriter, InputRef};
use crate::utils::ColIndexMapping;

pub struct ApplyProjectRule {}
impl Rule for ApplyProjectRule {
    fn apply(&self, plan: PlanRef) -> Option<PlanRef> {
        let apply = plan.as_logical_apply()?;
        let right = apply.right();
        let project = right.as_logical_project()?;
        let new_right = project.input();

        match apply.join_type() {
            JoinType::LeftOuter => {
                // For LeftOuter, we pull the LogicalProject up on top of LogicalApply.
                // Wrong!
                let mut shift_input_ref = ColIndexMapping::with_shift_offset(
                    project.input().schema().len(),
                    apply.left().schema().len() as isize,
                );

                // All the columns in the left.
                let mut exprs: Vec<ExprImpl> = apply
                    .left()
                    .schema()
                    .fields()
                    .iter()
                    .enumerate()
                    .map(|(i, field)| InputRef::new(i, field.data_type()).into())
                    .collect();
                // Extend with the project columns in the right.
                exprs.extend(project.exprs().clone().into_iter().map(|expr| {
                    // We currently assume that there is no correlated variable in LogicalProject.
                    shift_input_ref.rewrite_expr(expr)
                }));
                let mut expr_alias = vec![None; apply.left().schema().len()];
                expr_alias.extend(project.expr_alias().iter().cloned());

                let new_apply = apply.clone_with_left_right(apply.left(), new_right);
                let lifted_project: PlanRef =
                    LogicalProject::new(new_apply.into(), exprs, expr_alias).into();
                println!("Apply LogicalProject for LeftOuter finished.");
                Some(lifted_project)
            }
            JoinType::LeftSemi => {
                // For LeftSemi, we just remove LogicalProject.
                let new_apply = apply.clone_with_left_right(apply.left(), new_right);
                Some(new_apply.into())
            }
            _ => Some(plan),
        }
    }
}

impl ApplyProjectRule {
    pub fn create() -> BoxedRule {
        Box::new(ApplyProjectRule {})
    }
}
