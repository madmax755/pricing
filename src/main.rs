use ndarray::{Array1, Array2, s};
use rand_distr::{Normal, Distribution};
use statrs::distribution::{Normal as StatNormal, ContinuousCDF};
use rand::rng;

struct GBM {
    spot: f64,
    drift: f64,
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
        let mut rng = rng();

        path[0] = self.spot;
        for i in 1..self.steps {
            // calculate the next step using GBM formula
            path[i] = path[i-1] * (1.0 + self.drift * dt + self.volatility * normal.sample(&mut rng) * dt.sqrt());
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

    fn run_simulation(&self, trials: usize, strike_price: f64,  payoff_function: fn(&f64, &f64) -> f64) -> f64 {

        let paths = self.generate_paths(trials);

        let spot_prices = paths.slice(s![..,-1]);

        let option_payoff = spot_prices.map(|x| payoff_function(x, &strike_price));

        option_payoff.sum()/trials as f64
    }

    // calculates the price of a European call option using the Black-Scholes formula (https://en.wikipedia.org/wiki/Black%E2%80%93Scholes_model#Black%E2%80%93Scholes_formula)
    fn price_using_black_scholes(&self, strike_price: f64) -> f64 {
        let normal = StatNormal::new(0.0, 1.0).unwrap();
        let d1 = (self.spot.ln() - strike_price.ln() + (self.drift + 0.5 * self.volatility * self.volatility) * self.time_to_maturity) / 
                 (self.volatility * self.time_to_maturity.sqrt());
        let d2 = d1 - self.volatility * self.time_to_maturity.sqrt();
        let call_price = self.spot * normal.cdf(d1) - strike_price * (-self.drift * self.time_to_maturity).exp() * normal.cdf(d2);
        call_price
    }
}


fn euro_call_payoff(spot_price: &f64, strike_price: &f64) -> f64 {
    (spot_price - strike_price).max(0.0)
}

fn main() {
    let model = GBM {
        spot: 100.0,
        drift: 0.0,
        volatility: 0.2,
        time_to_maturity: 1.0,
        steps: 100,
    };

    let strike_price: f64 = 120.0;

    let result = model.run_simulation(100000, strike_price, euro_call_payoff);

    println!("Monte Carlo Price: {}", result);

    let black_scholes_price = model.price_using_black_scholes(strike_price);

    println!("Black-Scholes Price: {}", black_scholes_price);
}


