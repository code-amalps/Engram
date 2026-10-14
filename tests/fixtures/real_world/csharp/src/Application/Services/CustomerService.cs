using Commerce.Domain.Entities;
using Commerce.Domain.Interfaces;

namespace Commerce.Application.Services;

public class CustomerService
{
    private readonly ICustomerRepository _repository;

    public CustomerService(ICustomerRepository repository)
    {
        _repository = repository;
    }

    public Customer RegisterCustomer(string id, string name, string email)
    {
        var customer = new Customer
        {
            Id = id,
            FullName = name,
            Email = email,
            IsActive = true
        };
        _repository.Save(customer);
        return customer;
    }

    public bool DeactivateCustomer(string id)
    {
        var customer = _repository.GetById(id);
        if (customer == null) return false;
        customer.Deactivate();
        _repository.Save(customer);
        return true;
    }
}
