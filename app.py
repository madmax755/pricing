from fastapi import FastAPI, Request, Form, WebSocket
from fastapi.templating import Jinja2Templates
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse, JSONResponse
import subprocess
import json
import os
import uvicorn
import asyncio
import time

app = FastAPI()

# set up templates
templates = Jinja2Templates(directory="templates")

# set up static files
static_files = StaticFiles(directory="static")
app.mount("/static", static_files)

@app.get("/", response_class=HTMLResponse)
async def index(request: Request):
    return templates.TemplateResponse("index.html", {"request": request})

@app.post("/api/greeks")
async def greeks(
    spot: float = Form(...),
    risk_free_rate: float = Form(...),
    volatility: float = Form(...),
    time_to_maturity: float = Form(...),
    steps: int = Form(...),
    strike_price: float = Form(...),
    option_type: str = Form(...)
) -> JSONResponse:
    
    # recieved parameters
    print(f"Received parameters: {spot}, {risk_free_rate}, {volatility}, {time_to_maturity}, {steps}, {strike_price}, {option_type}")
    
    # run the rust program with the provided parameters
    result = subprocess.run([
        "./target/release/gbm_option_pricing",
        "greeks",
        option_type,
        str(spot),
        str(risk_free_rate),
        str(volatility),
        str(time_to_maturity),
        str(steps),
        str(strike_price),
        "1" # dummy value for num_trials
    ], capture_output=True, text=True)

    # parse the output
    try:
        output = json.loads(result.stdout)
    except json.JSONDecodeError:
        return JSONResponse(status_code=500, content={"error": "Failed to parse output"})
    
    return JSONResponse(status_code=200, content=output)

@app.websocket("/ws/mc-simulation")
async def mc_simulation(websocket: WebSocket):
    await websocket.accept()
    
    try:
        # receive parameters from client
        params = await websocket.receive_json()
        
        # Ensure all parameters are properly formatted
        spot = float(params["spot"])
        risk_free_rate = float(params["risk_free_rate"])
        volatility = float(params["volatility"])
        time_to_maturity = float(params["time_to_maturity"])
        steps = int(params["steps"])
        strike_price = float(params["strike_price"])
        option_type = params["option_type"]

        print(str(spot), str(risk_free_rate), str(volatility), str(time_to_maturity), str(steps), str(strike_price), option_type)

        # calculate black-scholes price first (quick calculation)
        bs_process = subprocess.run([
            "./target/release/gbm_option_pricing",
            "bs_only",  # new mode flag
            option_type,
            str(spot),
            str(risk_free_rate),
            str(volatility),
            str(time_to_maturity),
            str(steps),
            str(strike_price),
            "1"  # dummy value for num_trials
        ], capture_output=True, text=True)
        
        try:
            bs_result = json.loads(bs_process.stdout)
            print(f"Black-scholes result: {bs_result}")
            black_scholes_price = bs_result["black_scholes_price"]
            
            # send black-scholes price to client
            await websocket.send_json({
                "type": "bs_price",
                "value": black_scholes_price
            })
            
            # now run the monte carlo simulation in smaller batches
            total_trials = int(params["num_trials"])
            batch_size = min(1000, total_trials // 1000)
            if batch_size == 0:
                batch_size = 1
            
            running_sum = 0
            trials_completed = 0
            
            while trials_completed < total_trials:
                current_batch = min(batch_size, total_trials - trials_completed)
                
                # run a batch of simulations
                batch_process = subprocess.run([
                    "./target/release/gbm_option_pricing",
                    "batch",  # batch mode flag
                    option_type,
                    str(spot),
                    str(risk_free_rate),
                    str(volatility),
                    str(time_to_maturity),
                    str(steps),
                    str(strike_price),
                    str(current_batch)
                ], capture_output=True, text=True)
                
                
                if batch_process.stderr:
                    await websocket.send_json({
                        "error": f"Batch process error: {batch_process.stderr}"
                    })
                    break
                
                batch_result = json.loads(batch_process.stdout)
                running_sum += batch_result["batch_sum"]
                trials_completed += current_batch
                
                # calculate current monte carlo price
                current_price = running_sum / trials_completed
                
                # send update to client
                await websocket.send_json({
                    "type": "mc_update",
                    "trials": trials_completed,
                    "price": current_price
                })
                
            
            # send final result
            await websocket.send_json({
                "type": "complete",
                "trials": trials_completed,
                "final_price": running_sum / trials_completed
            })
        except json.JSONDecodeError as e:
            await websocket.send_json({
                "error": f"JSON decode error: {str(e)}, Output: {bs_process.stdout}, Error: {bs_process.stderr}"
            })
        
    except Exception as e:
        import traceback
        traceback_str = traceback.format_exc()
        print(f"Exception in websocket: {traceback_str}")
        await websocket.send_json({"error": f"{str(e)}\n{traceback_str}"})
    
    finally:
        await websocket.close()

@app.post("/simulate")
async def simulate(
    spot: float = Form(...),
    risk_free_rate: float = Form(...),
    volatility: float = Form(...),
    time_to_maturity: float = Form(...),
    steps: int = Form(...),
    strike_price: float = Form(...),
    num_trials: int = Form(...)
):
    # this endpoint is kept for backward compatibility
    # run the rust program with the provided parameters
    result = subprocess.run([
        "./target/release/gbm_option_pricing",
        "full",  # full mode flag
        str(spot),
        str(risk_free_rate),
        str(volatility),
        str(time_to_maturity),
        str(steps),
        str(strike_price),
        str(num_trials)
    ], capture_output=True, text=True)
    
    # parse the output
    try:
        output = json.loads(result.stdout)
    except json.JSONDecodeError:
        return {"error": "Failed to parse output", "raw_output": result.stdout}
    
    return output

@app.get("/ping")
async def ping():
    return {"status": "ok", "message": "Server is running"}

if __name__ == "__main__":
    # make sure the rust program is compiled
    subprocess.run(["cargo", "build", "--release"])
    
    uvicorn.run(app, host="0.0.0.0", port=8000) 