FROM python:3.13-slim

WORKDIR /app

COPY requirements.txt .
RUN pip install uv && uv pip install --system -r requirements.txt

COPY main.py .

EXPOSE 8093

CMD ["python", "main.py"]
