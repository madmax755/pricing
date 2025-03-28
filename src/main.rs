use ndarray::{Array1, Array2, s};
use rand_distr::{Normal, Distribution};
use statrs::distribution::{Normal as StatNormal, ContinuousCDF};
use rand::thread_rng;
use serde::Serialize;
use std::env;
use serde_json;

#[derive(Serialize)]
struct SimulationResult {
    monte_carlo_price: f64,
    black_scholes_price: f64,
    mc_prices: Vec<f64>,
    trial_counts: Vec<usize>,
}

#[derive(Serialize)]
struct BlackScholesResult {
    black_scholes_price: f64,
}

#[derive(Serialize)]
struct BatchResult {
    batch_sum: f64,
}

struct GBM {
    spot: f64,
    risk_free_rate: f64,
    volatility: f64,
    time_to_maturity: f64,
    steps: usize,
}

impl GBM {
    fn dt(&self) -> f64 {
        self.time_to_maturity / self.steps as f64
    }
    
    // generates a single path 
    fn generate_path(&self) -> Array1<f64> {
        let dt = self.dt();
        let mut path = Array1::zeros(self.steps);
        let normal = Normal::new(0.0, 1.0).unwrap();
        let mut rng = thread_rng();

        path[0] = self.spot;
        for i in 1..self.steps {
            // Standard GBM formula (exact solution to the SDE):
            // S(t+dt) = S(t) * exp((r - 0.5*σ²) * dt + σ * √dt * Z)
            let z = normal.sample(&mut rng);
            let drift_term = (self.risk_free_rate - 0.5 * self.volatility * self.volatility) * dt;
            let diffusion_term = self.volatility * dt.sqrt() * z;
            path[i] = path[i-1] * (drift_term + diffusion_term).exp();
        }

        path
    }

    // generates multiple paths
    fn generate_paths(&self, num_paths: usize) -> Array2<f64> {
        let mut paths = Array2::zeros((num_paths, self.steps));
        for i in 0..num_paths {
            paths.slice_mut(s![i, ..]).assign(&self.generate_path());
        }
        paths
    }

    fn run_simulation(&self, trials: usize, strike_price: f64, payoff_function: fn(&f64, &f64) -> f64) -> f64 {
        let paths = self.generate_paths(trials);
        let spot_prices = paths.slice(s![..,-1]);
        let option_payoff = spot_prices.map(|x| payoff_function(x, &strike_price));
        let discount_factor = (-self.risk_free_rate * self.time_to_maturity).exp();
        discount_factor * option_payoff.sum() / trials as f64
    }
    
    // run a batch of simulations and return the sum of payoffs
    fn run_batch(&self, batch_size: usize, strike_price: f64, payoff_function: fn(&f64, &f64) -> f64) -> f64 {
        let paths = self.generate_paths(batch_size);
        let spot_prices = paths.slice(s![..,-1]);
        let option_payoff = spot_prices.map(|x| payoff_function(x, &strike_price));
        let discount_factor = (-self.risk_free_rate * self.time_to_maturity).exp();
        discount_factor * option_payoff.sum()
    }
    
    // run simulation with intermediate results for convergence plotting
    fn run_simulation_with_tracking(&self, trials: usize, strike_price: f64, payoff_function: fn(&f64, &f64) -> f64) -> (f64, Vec<f64>, Vec<usize>) {
        // track intermediate results at logarithmically spaced intervals
        let mut mc_prices = Vec::new();
        let mut trial_counts = Vec::new();
        
        // determine checkpoints (logarithmically spaced)
        let num_checkpoints = 20;
        let mut checkpoints = Vec::new();
        for i in 0..=num_checkpoints {
            let checkpoint = (trials as f64 * (10.0_f64.powf(i as f64 / num_checkpoints as f64) / 10.0)) as usize;
            if checkpoint > 0 && checkpoint <= trials {
                checkpoints.push(checkpoint);
            }
        }
        if !checkpoints.contains(&trials) {
            checkpoints.push(trials);
        }
        
        // run simulation and track results at checkpoints
        let mut running_sum = 0.0;
        let mut paths_generated = 0;
        let discount_factor = (-self.risk_free_rate * self.time_to_maturity).exp();
        
        for checkpoint in checkpoints {
            let paths_to_generate = checkpoint - paths_generated;
            if paths_to_generate == 0 {
                continue;
            }
            
            let paths = self.generate_paths(paths_to_generate);
            let spot_prices = paths.slice(s![..,-1]);
            let option_payoffs = spot_prices.map(|x| payoff_function(x, &strike_price));
            
            running_sum += option_payoffs.sum();
            paths_generated = checkpoint;
            
            let current_price = discount_factor * running_sum / paths_generated as f64;
            mc_prices.push(current_price);
            trial_counts.push(paths_generated);
        }
        
        let final_price = discount_factor * running_sum / trials as f64;
        (final_price, mc_prices, trial_counts)
    }

    // calculates the price of a European call option using the Black-Scholes formula
    fn price_using_black_scholes(&self, strike_price: f64) -> f64 {
        let normal = StatNormal::new(0.0, 1.0).unwrap();
        let d1 = (self.spot.ln() - strike_price.ln() + (self.risk_free_rate + 0.5 * self.volatility * self.volatility) * self.time_to_maturity) / 
                 (self.volatility * self.time_to_maturity.sqrt());
        let d2 = d1 - self.volatility * self.time_to_maturity.sqrt();
        let call_price = self.spot * normal.cdf(d1) - strike_price * (-self.risk_free_rate * self.time_to_maturity).exp() * normal.cdf(d2);
        call_price
    }
}

fn euro_call_payoff(spot_price: &f64, strike_price: &f64) -> f64 {
    (spot_price - strike_price).max(0.0)
}

// Add this function to handle parsing errors
fn parse_arg<T: std::str::FromStr>(args: &[String], index: usize, name: &str) -> Result<T, String> 
where T::Err: std::fmt::Display 
{
    if index >= args.len() {
        return Err(format!("Missing argument for {}", name));
    }
    
    match args[index].parse::<T>() {
        Ok(value) => Ok(value),
        Err(e) => Err(format!("Invalid {}: {}", name, e))
    }
}

fn main() {
    // parse command line arguments
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 9 {
        eprintln!("Usage: {} <mode> <spot> <drift> <volatility> <time_to_maturity> <steps> <strike_price> <num_trials>", args[0]);
        eprintln!("Mode can be: full, bs_only, or batch");
        std::process::exit(1);
    }
    
    let mode = &args[1];
    
    // Use the new parse_arg function to handle errors gracefully
    let spot = match parse_arg::<f64>(&args, 2, "spot price") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let risk_free_rate = match parse_arg::<f64>(&args, 3, "risk-free rate") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let volatility = match parse_arg::<f64>(&args, 4, "volatility") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let time_to_maturity = match parse_arg::<f64>(&args, 5, "time to maturity") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let steps = match parse_arg::<usize>(&args, 6, "steps") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let strike_price = match parse_arg::<f64>(&args, 7, "strike price") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let num_trials = match parse_arg::<usize>(&args, 8, "number of trials") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let model = GBM {
        spot,
        risk_free_rate,
        volatility,
        time_to_maturity,
        steps,
    };

    // handle different modes
    match mode.as_str() {
        "full" => {
            // calculate Black-Scholes price
            let black_scholes_price = model.price_using_black_scholes(strike_price);
            
            // run Monte Carlo simulation with tracking
            let (monte_carlo_price, mc_prices, trial_counts) = 
                model.run_simulation_with_tracking(num_trials, strike_price, euro_call_payoff);
            
            // output results as JSON
            let result = SimulationResult {
                monte_carlo_price,
                black_scholes_price,
                mc_prices,
                trial_counts,
            };
            
            println!("{}", serde_json::to_string(&result).unwrap());
        },
        "bs_only" => {
            // only calculate Black-Scholes price
            let black_scholes_price = model.price_using_black_scholes(strike_price);
            
            let result = BlackScholesResult {
                black_scholes_price,
            };
            
            println!("{}", serde_json::to_string(&result).unwrap());
        },
        "batch" => {
            // run a batch of simulations and return the sum
            let batch_sum = model.run_batch(num_trials, strike_price, euro_call_payoff);
            
            let result = BatchResult {
                batch_sum,
            };
            
            println!("{}", serde_json::to_string(&result).unwrap());
        },
        _ => {
            eprintln!("Invalid mode: {}. Use 'full', 'bs_only', or 'batch'", mode);
            std::process::exit(1);
        }
    }
}


