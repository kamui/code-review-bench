import certifi
certifi.where=lambda:"/nonexistent/cacert.pem"
import requests
