use ndarray::{Array1, Array2, s};
use rand_distr::{Normal, Distribution};
use statrs::distribution::{Normal as StatNormal, ContinuousCDF};
use rand::thread_rng;
pub struct GBM {
    pub spot: f64,
    pub risk_free_rate: f64,
    pub volatility: f64,
    pub time_to_maturity: f64,
    pub steps: usize,
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

    // runs a single simulation and returns the price calculated
    pub fn run_simulation(&self, trials: usize, strike_price: f64, payoff_function: fn(&f64, &f64) -> f64) -> f64 {
        let paths = self.generate_paths(trials);
        let spot_prices = paths.slice(s![..,-1]);
        let option_payoff = spot_prices.map(|x| payoff_function(x, &strike_price));
        let discount_factor = (-self.risk_free_rate * self.time_to_maturity).exp();
        discount_factor * option_payoff.sum() / trials as f64
    }
    
    // run a batch of simulations and return the sum of payoffs
    pub fn run_batch(&self, batch_size: usize, strike_price: f64, payoff_function: fn(&f64, &f64) -> f64) -> f64 {
        let paths = self.generate_paths(batch_size);
        let spot_prices = paths.slice(s![..,-1]);
        let option_payoff = spot_prices.map(|x| payoff_function(x, &strike_price));
        let discount_factor = (-self.risk_free_rate * self.time_to_maturity).exp();
        discount_factor * option_payoff.sum()
    }
    
    // run simulation with intermediate results for convergence plotting
    pub fn run_simulation_with_tracking(&self, trials: usize, strike_price: f64, payoff_function: fn(&f64, &f64) -> f64) -> (f64, Vec<f64>, Vec<usize>) {
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
    pub fn price_using_black_scholes(&self, strike_price: f64) -> f64 {
        let normal = StatNormal::new(0.0, 1.0).unwrap();
        let d1 = (self.spot.ln() - strike_price.ln() + (self.risk_free_rate + 0.5 * self.volatility * self.volatility) * self.time_to_maturity) / 
                 (self.volatility * self.time_to_maturity.sqrt());
        let d2 = d1 - self.volatility * self.time_to_maturity.sqrt();
        let call_price = self.spot * normal.cdf(d1) - strike_price * (-self.risk_free_rate * self.time_to_maturity).exp() * normal.cdf(d2);
        call_price
    }
}
