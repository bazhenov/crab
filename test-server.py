# Python 3 server example
from http.server import BaseHTTPRequestHandler, HTTPServer
import time

hostName = "localhost"
serverPort = 8080

class MyServer(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/":
            self.send_response(301)
            self.send_header("Location", "http://localhost:8080/page/1")
            self.end_headers()

        elif self.path.startswith("/page/"):
            page = int(self.path[6:])
            if page < 1 or page > 100:
                self.send_response(400)
                self.end_headers()
            
            else:
                self.send_response(200)
                self.send_header("Content-type", "text/html")
                self.end_headers()

                self.wfile.write(bytes("<html>", "utf-8"))
                self.wfile.write(bytes("<body>", "utf-8"))
                self.wfile.write(bytes("<p>Page number %d</p>" % page, "utf-8"))

                self.wfile.write(bytes("<ul> ", "utf-8"))
                for d in range(0, 10):
                    self.wfile.write(bytes("<li><a href='/data/%d'>Data %d</a></li>" % (page * 10 + d, page * 10 + d), "utf-8"))
                self.wfile.write(bytes("</ul> ", "utf-8"))

                self.wfile.write(bytes("<section class='pager'> ", "utf-8"))
                for p in range(max(1, page - 5), min(100, page + 5)):
                    self.wfile.write(bytes("<a href='/page/%d'>Page %d</a> " % (p, p), "utf-8"))
                self.wfile.write(bytes("</section> ", "utf-8"))

                self.wfile.write(bytes("</body></html>", "utf-8"))

        elif self.path.startswith("/data/"):
            d = int(self.path[6:])
            self.send_response(200)
            self.send_header("Content-type", "text/html")
            self.end_headers()

            self.wfile.write(bytes("<html>", "utf-8"))
            self.wfile.write(bytes("<body>", "utf-8"))
            self.wfile.write(bytes("<p>Data for <span class='input'>%d</span> is <span class='output'>%d</span></p>" % (d, d**2), "utf-8"))
            self.wfile.write(bytes("</body></html>", "utf-8"))
        
        else:
            self.send_response(404)
            self.end_headers()

if __name__ == "__main__":        
    webServer = HTTPServer((hostName, serverPort), MyServer)
    print("Server started http://%s:%s" % (hostName, serverPort))

    try:
        webServer.serve_forever()
    except KeyboardInterrupt:
        pass

    webServer.server_close()
    print("Server stopped.")
