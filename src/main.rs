use serde::Serialize;
use std::env;
use serde_json;

mod gbm;
use crate::gbm::GBM;

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

#[derive(Serialize)]
struct GreeksResult {
    delta: f64,
    gamma: f64,
    theta: f64,
    vega: f64,
}

#[derive(Serialize)]
struct PriceDistributionResult {
    prices: Vec<f64>,
}


fn euro_call_payoff(spot_price: &f64, strike_price: &f64) -> f64 {
    (spot_price - strike_price).max(0.0)
}

fn euro_put_payoff(spot_price: &f64, strike_price: &f64) -> f64 {
    (strike_price - spot_price).max(0.0)
}

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
        eprintln!("Usage: {} <mode> <option_type> <spot> <drift> <volatility> <time_to_maturity> <steps> <strike_price> <num_trials>", args[0]);
        eprintln!("Mode can be: full, bs_only, batch, or distribution");
        eprintln!("Option type can be: call or put");
        std::process::exit(1);
    }
    
    let mode = &args[1];

    let option_type = &args[2];
    
    let spot = match parse_arg::<f64>(&args, 3, "spot price") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let risk_free_rate = match parse_arg::<f64>(&args, 4, "risk-free rate") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let volatility = match parse_arg::<f64>(&args, 5, "volatility") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let time_to_maturity = match parse_arg::<f64>(&args, 6, "time to maturity") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let steps = match parse_arg::<usize>(&args, 7, "steps") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let strike_price = match parse_arg::<f64>(&args, 8, "strike price") {
        Ok(val) => val,
        Err(e) => {
            let error = serde_json::json!({
                "error": e
            });
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };
    
    let num_trials = match parse_arg::<usize>(&args, 9, "number of trials") {
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
            if option_type == "call" {

                // calculate Black-Scholes price
                let black_scholes_price = model.price_call_bs(strike_price);
                
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
            } else {
                // calculate Black-Scholes price
                let black_scholes_price = model.price_put_bs(strike_price);
                
                // run Monte Carlo simulation with tracking
                let (monte_carlo_price, mc_prices, trial_counts) = 
                    model.run_simulation_with_tracking(num_trials, strike_price, euro_put_payoff);
                
                // output results as JSON
                let result = SimulationResult {
                    monte_carlo_price,
                    black_scholes_price,
                    mc_prices,
                    trial_counts,
                };
            
                println!("{}", serde_json::to_string(&result).unwrap());
            }
        },
        "bs_only" => {

            if option_type == "call" {
                // only calculate Black-Scholes price
                let black_scholes_price = model.price_call_bs(strike_price);
                let result = BlackScholesResult {
                    black_scholes_price,
                };
                
                println!("{}", serde_json::to_string(&result).unwrap());

            } else {
                // only calculate Black-Scholes price
                let black_scholes_price = model.price_put_bs(strike_price);
                let result = BlackScholesResult {
                    black_scholes_price,
                };
                
                println!("{}", serde_json::to_string(&result).unwrap());
            }
            
            
        },
        "batch" => {
            if option_type == "call" {
                // run a batch of simulations and return the sum
                let batch_sum = model.run_batch(num_trials, strike_price, euro_call_payoff);
                
                let result = BatchResult {
                    batch_sum,
                };
                
                println!("{}", serde_json::to_string(&result).unwrap());
            } else {
                // run a batch of simulations and return the sum
                let batch_sum = model.run_batch(num_trials, strike_price, euro_put_payoff);
                
                let result = BatchResult {
                    batch_sum,
                };
                
                println!("{}", serde_json::to_string(&result).unwrap());
            }
        },

        "distribution" => {
            // Generate price distribution
            let prices = model.generate_price_distribution(num_trials);
            
            let result = PriceDistributionResult {
                prices,
            };
            
            println!("{}", serde_json::to_string(&result).unwrap());
        },

        "greeks" => {
            
            // Calculate Greeks
            let (delta, gamma, theta, vega) = model.calculate_greeks(strike_price, &option_type);
            
            let result = GreeksResult {
                delta,
                gamma,
                theta,
                vega,
            };
            
            println!("{}", serde_json::to_string(&result).unwrap());
        },
        _ => {
            eprintln!("Invalid mode: {}. Use 'full', 'bs_only', or 'batch'", mode);
            std::process::exit(1);
        }
    }
}


