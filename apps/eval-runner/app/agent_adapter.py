from pydantic import BaseModel


class AgentAdapterSpec(BaseModel):
    name: str
    version: str
    command: str | None = None
    supports_trajectory: bool = True
