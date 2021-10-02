use crate::construction::heuristics::*;
use crate::models::problem::Job;
use crate::solver::mutation::LocalOperator;
use crate::solver::RefinementContext;
use crate::utils::unwrap_from_result;
use hashbrown::HashSet;

const MIN_JOBS: usize = 2;

/// A local search operator which tries to exchange sequence of jobs between routes.
pub struct ExchangeSequence {
    max_sequence_size: usize,
}

impl LocalOperator for ExchangeSequence {
    fn explore(&self, _: &RefinementContext, insertion_ctx: &InsertionContext) -> Option<InsertionContext> {
        let route_indices = insertion_ctx
            .solution
            .routes
            .iter()
            .enumerate()
            .filter_map(|(idx, route_ctx)| {
                let has_locked_jobs =
                    route_ctx.route.tour.jobs().any(|job| insertion_ctx.solution.locked.contains(&job));
                let has_enough_jobs = route_ctx.route.tour.job_count() >= MIN_JOBS;

                if has_locked_jobs || has_enough_jobs {
                    None
                } else {
                    Some(idx)
                }
            })
            .collect::<Vec<_>>();

        if route_indices.is_empty() {
            return None;
        }

        let mut insertion_ctx = insertion_ctx.deep_copy();

        exchange_jobs(&mut insertion_ctx, route_indices.as_slice(), self.max_sequence_size);

        Some(insertion_ctx)
    }
}

fn exchange_jobs(insertion_ctx: &mut InsertionContext, route_indices: &[usize], max_sequence_size: usize) {
    let get_route_idx = || {
        let idx = insertion_ctx.environment.random.uniform_int(0, route_indices.len() as i32) as usize;
        route_indices.get(idx).cloned().unwrap()
    };

    let get_sequence_size = |insertion_ctx: &InsertionContext, route_idx: usize| {
        let job_count = get_route_ctx(insertion_ctx, route_idx).route.tour.job_count().min(max_sequence_size);
        insertion_ctx.environment.random.uniform_int(MIN_JOBS as i32, job_count as i32) as usize
    };

    let first_route_idx = get_route_idx();
    let first_sequence_size = get_sequence_size(insertion_ctx, first_route_idx);

    let second_route_idx = get_route_idx();
    let second_sequence_size = get_sequence_size(insertion_ctx, second_route_idx);

    let first_jobs = extract_jobs(insertion_ctx, first_route_idx, first_sequence_size);
    let second_jobs = extract_jobs(insertion_ctx, second_route_idx, second_sequence_size);

    insert_jobs(insertion_ctx, first_route_idx, second_jobs);
    insert_jobs(insertion_ctx, second_route_idx, first_jobs);

    finalize_insertion_ctx(insertion_ctx);
}

fn extract_jobs(insertion_ctx: &mut InsertionContext, route_idx: usize, sequence_size: usize) -> Vec<Job> {
    let route_ctx = insertion_ctx.solution.routes.get_mut(route_idx).unwrap();
    let job_count = route_ctx.route.tour.job_count();

    assert!(job_count > sequence_size);

    // get jobs in the exact order as they appear first time in the tour
    let (_, jobs) = route_ctx.route.tour.all_activities().filter_map(|activity| activity.retrieve_job()).fold(
        (HashSet::<Job>::default(), Vec::with_capacity(job_count)),
        |(mut set, mut vec), job| {
            if !set.contains(&job) {
                vec.push(job)
            } else {
                set.insert(job);
            }

            (set, vec)
        },
    );

    assert_eq!(jobs.len(), job_count);

    let last_index = job_count - sequence_size;
    let start_index = insertion_ctx.environment.random.uniform_int(1, last_index as i32) as usize;

    (start_index..(start_index + sequence_size)).for_each(|index| {
        let job = jobs.get(index).unwrap();
        assert!(route_ctx.route_mut().tour.remove(job));
    });

    insertion_ctx.problem.constraint.accept_route_state(route_ctx);

    jobs
}

fn insert_jobs(insertion_ctx: &mut InsertionContext, route_idx: usize, jobs: Vec<Job>) {
    let random = &insertion_ctx.environment.random;
    let result_selector = BestResultSelector::default();

    let start_index =
        random.uniform_int(0, get_route_ctx(insertion_ctx, route_idx).route.tour.job_activity_count() as i32) as usize;

    let (failures, _) = jobs.into_iter().fold((Vec::new(), start_index), |(mut unassigned, start_index), job| {
        // reevaluate last insertion point
        let last_index = get_route_ctx(insertion_ctx, route_idx).route.tour.job_activity_count();
        // try to find success insertion starting from given point
        let (result, start_index) = unwrap_from_result((start_index..=last_index).try_fold(
            (InsertionResult::make_failure(), start_index),
            |_, insertion_idx| {
                let insertion = evaluate_job_insertion_in_route(
                    insertion_ctx,
                    get_route_ctx(insertion_ctx, route_idx),
                    &job,
                    InsertionPosition::Concrete(insertion_idx),
                    // NOTE we don't try to insert the best, so alternative is a failure
                    InsertionResult::make_failure(),
                    &result_selector,
                );

                match &insertion {
                    InsertionResult::Failure(_) => Ok((insertion, insertion_idx)),
                    InsertionResult::Success(_) => Err((insertion, insertion_idx)),
                }
            },
        ));

        match result {
            InsertionResult::Success(success) => {
                apply_insertion_success(insertion_ctx, success);
            }
            InsertionResult::Failure(failure) => unassigned.push(failure),
        }

        (unassigned, start_index + 1)
    });

    insertion_ctx
        .solution
        .unassigned
        .extend(failures.into_iter().map(|failure| (failure.job.unwrap(), failure.constraint)));
}

fn get_route_ctx(insertion_ctx: &InsertionContext, route_idx: usize) -> &RouteContext {
    insertion_ctx.solution.routes.get(route_idx).unwrap()
}
