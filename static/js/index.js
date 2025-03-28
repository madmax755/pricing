// initialize chart
const ctx = document.getElementById('convergenceChart').getContext('2d');
Chart.defaults.color = '#ffffff';
Chart.defaults.borderColor = '#444';

const chart = new Chart(ctx, {
    type: 'line',
    data: {
        labels: [],
        datasets: [
            {
                label: 'Monte Carlo Price',
                data: [],
                borderColor: '#55a490',
                backgroundColor: 'rgba(85, 164, 144, 0.1)',
                tension: 0.2,
                borderWidth: 2,
                pointRadius: 0,
                pointHoverRadius: 4
            },
            {
                label: 'Black-Scholes Price',
                data: [],
                borderColor: '#e05c5c',
                borderWidth: 2,
                borderDash: [5, 5],
                pointRadius: 0,
                fill: false
            }
        ]
    },
    options: {
        responsive: true,
        maintainAspectRatio: false,
        animation: false,
        scales: {
            x: {
                title: {
                    display: true,
                    text: 'Number of Trials',
                    color: '#ffffff',
                    font: {
                        size: 14
                    }
                },
                grid: {
                    color: 'rgba(68, 68, 68, 0.5)',
                    drawBorder: false
                }
            },
            y: {
                title: {
                    display: true,
                    text: 'Option Price',
                    color: '#ffffff',
                    font: {
                        size: 14
                    }
                },
                grid: {
                    color: 'rgba(68, 68, 68, 0.5)',
                    drawBorder: false
                }
            }
        },
        plugins: {
            legend: {
                labels: {
                    font: {
                        size: 12
                    },
                    padding: 20
                }
            }
        }
    }
});

let socket = null;
let blackScholesPrice = null;
let mcPrices = [];
let trialCounts = [];

document.getElementById('simulationForm').addEventListener('submit', async function (e) {
    e.preventDefault();

    // disable the run button
    const runButton = document.getElementById('runButton');
    runButton.disabled = true;
    runButton.classList.add('opacity-50');
    runButton.classList.remove('hover:bg-[#3c3c3c]');

    // reset chart and results
    chart.data.labels = [];
    chart.data.datasets[0].data = [];
    chart.data.datasets[1].data = [];
    chart.update();

    // Explicitly reset progress bar to 0%
    const progressBar = document.getElementById('progressBar');
    progressBar.style.width = '0%';

    // reset tracking variables
    blackScholesPrice = null;
    mcPrices = [];
    trialCounts = [];

    // reset Greeks display
    document.getElementById('deltaStat').textContent = '-';
    document.getElementById('gammaStat').textContent = '-';
    document.getElementById('thetaStat').textContent = '-';
    document.getElementById('vegaStat').textContent = '-';

    // get form data
    const formData = new FormData(this);
    const params = {};
    for (const [key, value] of formData.entries()) {
        params[key] = value;
    }

    // get greeks
    let formDataGreeks = new FormData(this);
    const responseGreeks = await fetch(`/api/greeks`, {
        method: 'POST',
        body: formDataGreeks
    });
    const dataGreeks = await responseGreeks.json();
    
    // Update Greeks display
    if (dataGreeks) {
        // Format the values to 4 decimal places with + sign for positive values
        const formatGreek = (value) => {
            const formatted = Math.abs(value) < 0.0001 ? value.toExponential(2) : value.toFixed(4);
            return value > 0 ? '+' + formatted : formatted;
        };

        document.getElementById('deltaStat').textContent = formatGreek(dataGreeks.delta);
        document.getElementById('gammaStat').textContent = formatGreek(dataGreeks.gamma);
        
        // For theta, express in daily terms and multiply by -1 to show decay
        const thetaDaily = dataGreeks.theta / 365;
        document.getElementById('thetaStat').textContent = formatGreek(thetaDaily);
        
        document.getElementById('vegaStat').textContent = formatGreek(dataGreeks.vega);
        
        // Add color coding based on value
        document.getElementById('deltaStat').className = 
            dataGreeks.delta > 0 ? 'text-2xl font-light mt-2 text-green-400' : 
            dataGreeks.delta < 0 ? 'text-2xl font-light mt-2 text-red-400' : 
            'text-2xl font-light mt-2';
            
        document.getElementById('thetaStat').className = 
            thetaDaily > 0 ? 'text-2xl font-light mt-2 text-green-400' : 
            thetaDaily < 0 ? 'text-2xl font-light mt-2 text-red-400' : 
            'text-2xl font-light mt-2';
    }

    document.getElementById('status').textContent = 'Connecting...';

    // close existing socket if any
    if (socket && socket.readyState < 2) {
        socket.close();
    }

    // create new WebSocket connection
    socket = new WebSocket(`ws://${window.location.host}/ws/mc-simulation`);

    socket.onopen = function () {
        document.getElementById('status').textContent = 'Connected. Starting simulation...';
        socket.send(JSON.stringify(params));
    };

    socket.onmessage = function (event) {
        const data = JSON.parse(event.data);

        if (data.error) {
            document.getElementById('status').textContent = 'Error: ' + data.error;
            runButton.disabled = false;
            runButton.classList.remove('opacity-50');
            runButton.classList.add('hover:bg-[#3c3c3c]');
            return;
        }

        if (data.type === 'bs_price') {
            // received Black-Scholes price
            blackScholesPrice = data.value;
            document.getElementById('status').textContent = 'Running Monte Carlo simulation...';
        }
        else if (data.type === 'mc_update') {
            // received Monte Carlo update
            const trials = data.trials;
            const price = data.price;

            // update progress
            const totalTrials = parseInt(params.num_trials);
            // Ensure trials is not greater than totalTrials
            const safeTrials = Math.min(trials, totalTrials);
            const progress = (safeTrials / totalTrials) * 100;
            document.getElementById('progressBar').style.width = progress + '%';

            // update chart
            trialCounts.push(trials);
            mcPrices.push(price);

            chart.data.labels = trialCounts;
            chart.data.datasets[0].data = mcPrices;

            if (blackScholesPrice !== null) {
                chart.data.datasets[1].data = Array(trialCounts.length).fill(blackScholesPrice);
            }

            chart.update();

            document.getElementById('status').textContent =
                `Running Monte Carlo simulation... ${trials} trials completed`;
        }
        else if (data.type === 'complete') {
            // simulation complete
            document.getElementById('status').textContent = 'Simulation complete!';
            runButton.disabled = false;
            runButton.classList.remove('opacity-50');
            runButton.classList.add('hover:bg-[#3c3c3c]');
        }
    };

    socket.onerror = function (error) {
        document.getElementById('status').textContent = 'WebSocket error!';
        console.error('WebSocket error:', error);
        runButton.disabled = false;
        runButton.classList.remove('opacity-50');
        runButton.classList.add('hover:bg-[#3c3c3c]');
    };

    socket.onclose = function () {
        document.getElementById('status').textContent += ' Connection closed.';
        runButton.disabled = false;
        runButton.classList.remove('opacity-50');
        runButton.classList.add('hover:bg-[#3c3c3c]');
    };
});