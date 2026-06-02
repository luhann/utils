from fastapi import FastAPI

app = FastAPI(title="Envy Data API", version="0.1.0")

@app.get("/")
def health_check():
    return {"status": "online", "environment": "envy-workspace"}

@app.get("/predict")
def get_prediction():
    # TODO: Hook up your trained model or processed data here
    return {"prediction": [0.85, 0.12, 0.44]}
