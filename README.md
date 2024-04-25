CommandRunner Struct
The CommandRunner is a Rust struct used for executing and handling expressions. It contains a values field, which is a thread-safe hash map for storing variable names (strings) and their values (CellValue).

'''Methods'''
`new`
new is an associated function used to create a new instance of CommandRunner. It takes an Arc<Mutex<HashMap<String, CellValue>>> as an argument, which is a thread-safe hash map for storing variable names and their values.

`run`
run is a method used to execute an expression. It takes a string as an argument, which is the expression to be executed.

If the expression can be parsed into a floating-point number, it returns this number. Otherwise, it uses a regular expression to parse the expression. If the parsing is successful, it performs the corresponding operation (addition, subtraction, multiplication, or division) and returns the result. If the parsing fails, it returns an error CellValue.

`eval_operand`
eval_operand is a method used to parse an operand. It takes a string as an argument, which is the operand to be parsed.

This method first locks the values field, then tries to get the value of the operand from values. If successful, it returns a clone of this value. If it fails, it returns an error CellValue.

`Example`
Here is an example of how to use CommandRunner:

`let values = Arc::new(Mutex::new(HashMap::new()));\n`
`let runner = CommandRunner::new(values);\n`
`let result = runner.run("1 + 2");\n`
(not test yet, may can not work. better use cargo test)
